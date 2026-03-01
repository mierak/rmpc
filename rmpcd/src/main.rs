use std::sync::Arc;

use anyhow::Result;
use mlua::Function;
use rmpc_mpd::{
    client::Client,
    commands::{IdleEvent, Status},
    mpd_client::MpdClient,
};
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::{async_client::AsyncClient, ctx::Ctx};

mod async_client;
mod ctx;
mod event_loop;
mod ext;
mod lua;
mod mpd_ext;
mod mpris;
mod song;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_writer(std::io::stderr).with_ansi(true).init();

    let (lua, lua_config) = lua::init()?;

    let on_song_change: Option<Function> = lua_config.get("on_song_change")?;
    let on_state_change: Option<Function> = lua_config.get("on_state_change")?;
    let address = lua_config.get::<String>("address")?;
    let password = lua_config.get::<Option<String>>("password")?;
    let (address, password) = rmpc_mpd::address::resolve(None, None, address, password);

    let (idle_tx, idle_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();

    let idle_tx_clone = idle_tx.clone();
    let mpd = Arc::new(AsyncClient::new(
        Client::init(address.clone(), password.clone(), "", None, false)?,
        move |evs| {
            if let Err(err) = idle_tx_clone.send(AppEvent::Idle(evs)) {
                error!(err = ?err, "Failed to send idle event");
            }
        },
    ));

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

    info!("Starting event loop");
    event_loop::init(
        mpd.clone(),
        ctx.clone(),
        idle_rx,
        idle_tx,
        tx,
        lua,
        on_song_change,
        on_state_change,
    )
    .await?;

    mpd.shutdown().await;

    Ok(())
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum AppEvent {
    Idle(Vec<IdleEvent>),
    StatusUpdate(Status),
}
