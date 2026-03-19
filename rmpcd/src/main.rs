use std::sync::Arc;

use anyhow::{Result, bail};
use rmpc_mpd::{
    commands::{IdleEvent, Status},
    mpd_client::MpdClient,
};
use rmpc_shared::paths::rmpcd_config_dir;
use tokio::sync::RwLock;
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

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

#[tokio::main]
async fn main() -> Result<()> {
    let start = std::time::Instant::now();

    tracing_subscriber::fmt()
        .with_line_number(true)
        .with_target(false)
        .with_file(true)
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy()
                .add_directive("rmpcd=debug".parse()?),
        )
        .init();

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
