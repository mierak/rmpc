use std::{
    fs::TryLockError,
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use clap::{Parser, Subcommand};
use rmpc_mpd::{
    commands::{IdleEvent, Status},
    mpd_client::{AlbumArtOrder, MpdClient},
};
use serde::Serialize;
use serde_json::json;
use tokio::sync::RwLock;
use tracing::{error, info, level_filters::LevelFilter, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt};

use crate::{
    async_client::AsyncClient,
    ctx::Ctx,
    lua::{eval_config, plugin::PluginStore},
    mpd_ext::MpdExt,
    paths::Paths,
    pkg::Lockfile,
};

mod async_client;
mod ctx;
mod event_loop;
mod ext;
mod kv_bridge;
mod lua;
mod mpd_ext;
mod mpris;
mod paths;
mod pkg;
mod subscribed_channels;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Clone, Debug, PartialEq)]
#[clap(rename_all = "lower")]
enum Command {
    /// Sets up a new config directory with example init.lua and .luarc.json
    /// config file for `LuaLS`
    Init,
    Pkg {
        #[command(subcommand)]
        command: PkgCommand,
    },
}

#[derive(Subcommand, Clone, Debug, PartialEq)]
#[clap(rename_all = "lower")]
enum PkgCommand {
    Upgrade,
}

fn init_logging(level: &str) -> Result<(WorkerGuard, WorkerGuard)> {
    let uid = rustix::process::geteuid();
    let file_appender = tracing_appender::rolling::never(
        std::env::temp_dir(),
        format!("rmpcd_{}.log", uid.as_raw()),
    );
    let (non_blocking_file, file_guard) = tracing_appender::non_blocking(file_appender);
    let (non_blocking_stderr, stderr_guard) = tracing_appender::non_blocking(std::io::stderr());

    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy()
        .add_directive(format!("rmpcd={level}").parse()?);

    let subscriber = Registry::default()
        .with(
            tracing_subscriber::fmt::Layer::new()
                .with_line_number(true)
                .with_target(false)
                .with_file(true)
                .with_ansi(true)
                .with_writer(non_blocking_stderr),
        )
        .with(
            tracing_subscriber::fmt::Layer::new()
                .with_line_number(true)
                .with_target(false)
                .with_file(true)
                .with_ansi(false)
                .with_writer(non_blocking_file),
        )
        .with(env_filter);

    tracing::subscriber::set_global_default(subscriber)?;
    log::set_boxed_logger(Box::new(kv_bridge::KvBridgeLogger::new()))?;
    log::set_max_level(log::LevelFilter::Trace);

    Ok((file_guard, stderr_guard))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    Paths::init()?;

    let lock_handle = match std::fs::File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .open(Paths::runtime_dir().join("instance.lock"))
    {
        Ok(v) => v,
        Err(err) => {
            eprintln!("Failed to open lock file: {err:?}");
            std::process::exit(1);
        }
    };

    match lock_handle.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            eprintln!("Another instance of rmpcd is already running, exiting...");
            std::process::exit(1);
        }
        err @ Err(_) => err?,
    }

    match args.command {
        Some(Command::Init) => run_init()?,
        Some(Command::Pkg { command }) => pkg::run_pkg(command).await?,
        None => run().await?,
    }

    Ok(())
}

fn run_init() -> Result<()> {
    let _log_guards = init_logging("info")?;

    let cfg_dir = Paths::config_dir();
    let init_lua_path = cfg_dir.join("init.lua");

    if init_lua_path.exists() {
        warn!("Config directory already exists at '{}', exiting...", cfg_dir.display());
        std::process::exit(1);
    }

    let default_config = include_str!("../../assets/rmpcd/example_init.lua");
    std::fs::write(&init_lua_path, default_config)?;

    info!("Created default config at '{}'", init_lua_path.display());

    match lua::type_def_eject::eject() {
        Ok(path) => {
            let value = &json!({
                "workspace.library": [path.display().to_string()]
            });
            let buf = Vec::new();
            let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
            let mut ser = serde_json::Serializer::with_formatter(buf, formatter);
            value.serialize(&mut ser)?;
            let luarc = String::from_utf8(ser.into_inner())?;

            std::fs::write(cfg_dir.join(".luarc.json"), luarc)?;

            info!("Created Lua API type definitions at '{}'", path.display());
        }
        Err(err) => {
            error!("Failed to eject Lua type definitions. {err:?}");
        }
    }

    Ok(())
}

async fn run() -> Result<()> {
    let start = std::time::Instant::now();
    let _log_guards = init_logging("debug")?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);
    ctrlc::set_handler(move || {
        let _ = shutdown_tx.send(());
    })?;

    let (idle_tx, idle_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();

    let idle_tx_clone = idle_tx.clone();
    let idle_tx_clone2 = idle_tx.clone();

    let mpd = Arc::new(AsyncClient::new(
        move |evs| {
            if let Err(err) = idle_tx_clone.send(AppEvent::Idle(evs)) {
                error!(err = ?err, "Failed to send idle event");
            }
        },
        move || {
            if let Err(err) = idle_tx_clone2.send(AppEvent::Reconnected) {
                error!(err = ?err, "Failed to send reconnected event");
            }
        },
    ));

    let cfg_dir = Paths::config_dir();

    let (_lua, lua_config, plugins) = eval_config(Some(mpd.clone())).await?;
    prepare_album_art_cache_dir(Paths::albumart_cache_dir()).await?;

    if let Err(err) = lua::type_def_eject::eject() {
        error!(err = ?err, "Failed to eject Lua type definitions");
    }

    let address = lua_config.get::<String>("address")?;
    let password = lua_config.get::<Option<String>>("password")?;
    let (address, password) = rmpc_mpd::address::resolve(None, None, address, password);
    let subscribed_channels =
        lua_config.get::<Option<Vec<String>>>("subscribed_channels")?.unwrap_or_default();
    let enable_keepalive = lua_config.get::<Option<bool>>("enable_keepalive")?.unwrap_or(true);

    mpd.connect(address, password, enable_keepalive).await?;

    let mut plugin_store = PluginStore::new();
    let mut lockfile = Lockfile::read_or_default().await?;
    for plugin in plugins.read().await.iter() {
        info!(path = ?plugin.read().await, "Loading plugin");
        match lua::plugin::load(cfg_dir, plugin, &mpd, &mut lockfile).await {
            Ok(plugin) => {
                info!(?plugin, "Successfully loaded plugin");
                plugin_store.insert(plugin.triggers, plugin);
            }
            Err(err) => {
                error!(err = ?err, "Failed to load plugin");
            }
        }
    }
    lockfile.write().await?;

    for channel in
        plugin_store.all().flat_map(|p| &p.subscribed_channels).chain(subscribed_channels.iter())
    {
        info!(channel, "Subscribing to channel");
        mpd.subscribe(channel.clone()).await?;
    }

    let status = mpd.run(|c| c.get_status()).await?;
    let current_song = mpd.run(|c| c.get_current_song()).await?;
    let queue = mpd.run(|c| c.playlist_info()).await?.unwrap_or_default();
    let album_art = match &current_song {
        Some(song) => {
            let uri = song.file.clone();
            mpd.run(move |c| c.find_album_art(&uri, AlbumArtOrder::EmbeddedFirst)).await?
        }
        None => None,
    };
    let ctx = Arc::new(RwLock::new(Ctx {
        current_song: current_song.clone(),
        status: status.clone(),
        queue,
        album_art,
        last_written_album_art_song_uri: None,
    }));

    let enable_mpris = lua_config.get::<Option<bool>>("mpris")?.unwrap_or(false);
    let tx = if enable_mpris { Some(mpris::setup(mpd.clone(), ctx.clone()).await?) } else { None };

    info!("rmpcd started in {:.2?}", start.elapsed());
    event_loop::init(mpd.clone(), ctx.clone(), idle_rx, shutdown_rx, idle_tx, tx, plugin_store)
        .await?;

    mpd.shutdown().await;

    Ok(())
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
enum AppEvent {
    Idle(Vec<IdleEvent>),
    StatusUpdate(Status),
    Reconnected,
}

async fn prepare_album_art_cache_dir(dir: &Path) -> Result<()> {
    let max_age = std::time::Duration::from_secs(60 * 60 * 24 * 7); // 7 days
    let now = SystemTime::now();

    match tokio::fs::read_dir(dir).await {
        Ok(mut entries) => loop {
            match entries.next_entry().await {
                Ok(Some(entry)) => {
                    if entry.metadata().await.is_ok_and(|m| {
                        m.is_file()
                            && m.modified().is_ok_and(|modified| {
                                modified < (now.checked_sub(max_age).unwrap_or(UNIX_EPOCH))
                            })
                    }) && let Err(err) = tokio::fs::remove_file(entry.path()).await
                        && err.kind() != std::io::ErrorKind::NotFound
                    {
                        tracing::warn!(path = ?entry.path(), err = ?err, "Failed to prune stale album art file");
                    }
                }
                Ok(None) => break,
                Err(err) => {
                    tracing::warn!(err = ?err, "Failed to read album art cache entry during prune");
                    break;
                }
            }
        },
        Err(err) => {
            tracing::warn!(err = ?err, "Failed to read album art cache dir for pruning");
        }
    }

    Ok(())
}
