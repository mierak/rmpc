use std::sync::Arc;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use rmpc_mpd::{
    commands::{IdleEvent, Status},
    mpd_client::MpdClient,
};
use rmpc_shared::paths::rmpcd_config_dir;
use serde::Serialize;
use serde_json::json;
use tokio::sync::RwLock;
use tracing::{error, info, level_filters::LevelFilter, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    async_client::AsyncClient,
    ctx::Ctx,
    lua::plugin::{LuaPlugin, PluginStore},
};

mod async_client;
mod ctx;
mod event_loop;
mod ext;
mod lua;
mod mpd_ext;
mod mpris;

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
}

fn init_logging(level: &str) -> Result<(WorkerGuard, WorkerGuard)> {
    let file_appender = tracing_appender::rolling::hourly("/tmp", "rmpcd.log");
    let (non_blocking_file, file_guard) = tracing_appender::non_blocking(file_appender);
    let (non_blocking_stderr, stderr_guard) = tracing_appender::non_blocking(std::io::stderr());

    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy()
        .add_directive(format!("rmpcd={level}").parse()?);

    Registry::default()
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
        .with(env_filter)
        .init();

    Ok((file_guard, stderr_guard))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Some(Command::Init) => run_init()?,
        None => run().await?,
    }

    Ok(())
}

fn run_init() -> Result<()> {
    let _log_guards = init_logging("info")?;

    let Some(cfg_dir) = rmpcd_config_dir() else {
        error!("Could not determine config directory");
        std::process::exit(1);
    };

    if cfg_dir.exists() {
        warn!("Config directory already exists at '{}', exiting...", cfg_dir.display());
        std::process::exit(1);
    }

    std::fs::create_dir_all(&cfg_dir)?;

    let init_lua_path = cfg_dir.join("init.lua");
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

    let Some(cfg_dir) = rmpcd_config_dir() else {
        bail!("Could not determine config directory");
    };

    let plugins: Arc<RwLock<Vec<_>>> = Arc::new(RwLock::new(Vec::new()));
    let lua = lua::create(&cfg_dir, &mpd, Some(&plugins))?;
    let lua_config = lua::eval_config(&lua, &cfg_dir).await?;

    if let Err(err) = lua::type_def_eject::eject() {
        error!(err = ?err, "Failed to eject Lua type definitions");
    }

    let mut plugin_store = PluginStore::new();
    for plugin in plugins.read().await.iter() {
        info!(path = ?plugin.read().await.path, "Loading plugin");
        let plugin = LuaPlugin::load(&cfg_dir, plugin, &mpd).await?;
        info!(?plugin, "Successfully loaded plugin");
        plugin_store.insert(plugin.triggers, plugin);
    }

    let address = lua_config.get::<String>("address")?;
    let password = lua_config.get::<Option<String>>("password")?;
    let (address, password) = rmpc_mpd::address::resolve(None, None, address, password);
    let subscribed_channels =
        lua_config.get::<Option<Vec<String>>>("subscribed_channels")?.unwrap_or_default();

    mpd.connect(address, password).await?;

    for channel in
        plugin_store.all().flat_map(|p| &p.subscribed_channels).chain(subscribed_channels.iter())
    {
        info!(channel, "Subscribing to channel");
        let channel = channel.clone();
        mpd.run(move |c| c.subscribe(&channel)).await?;
    }

    let status = mpd.run(|c| c.get_status()).await?;
    let current_song = mpd.run(|c| c.get_current_song()).await?;
    let queue = mpd.run(|c| c.playlist_info()).await?.unwrap_or_default();
    let ctx = Arc::new(RwLock::new(Ctx {
        current_song: current_song.clone(),
        status: status.clone(),
        queue,
        album_art: None,
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
