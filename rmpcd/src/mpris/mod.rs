use std::sync::Arc;

use anyhow::Result;
use tokio::sync::{RwLock, mpsc::UnboundedSender};
use tracing::error;

use crate::{async_client::AsyncClient, ctx::Ctx};

mod metadata;
mod notify;
mod player;
mod root;
mod tracklist;
pub use notify::{Change, notify_consumer};
pub use player::Player;
pub use root::Root;
pub use tracklist::Tracklist;

pub async fn setup(
    client: Arc<AsyncClient>,
    mpd_state: Arc<RwLock<Ctx>>,
) -> Result<UnboundedSender<Change>> {
    let root = Root {};
    let tracklist = Tracklist::new(mpd_state.clone());
    let player = Player::new(mpd_state.clone(), client);

    let conn = zbus::connection::Builder::session()?
        .name("org.mpris.MediaPlayer2.mpd")?
        .serve_at("/org/mpris/MediaPlayer2", root)?
        .serve_at("/org/mpris/MediaPlayer2", player)?
        .serve_at("/org/mpris/MediaPlayer2", tracklist)?
        .build()
        .await?;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Change>();
    let conn_clone = conn.clone();

    tokio::spawn(async move {
        if let Err(err) = notify_consumer(&conn_clone, rx, mpd_state).await {
            error!(err = ?err, "Failed to start dbus notify consumer");
        }
    });

    Ok(tx)
}
