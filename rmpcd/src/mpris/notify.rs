use std::sync::Arc;

use anyhow::Result;
use tokio::sync::{RwLock, mpsc::UnboundedReceiver};
use tracing::{error, info};
use zbus::{Connection, zvariant::OwnedObjectPath};

use crate::{
    ctx::Ctx,
    mpris::{Player, Tracklist, metadata::SongExt},
};

#[derive(Debug, Clone, Copy)]
pub enum Change {
    Volume,
    PlaybackState,
    Metadata,
    Queue,
}

pub async fn notify_consumer(
    connection: &Connection,
    mut rx: UnboundedReceiver<Change>,
    ctx: Arc<RwLock<Ctx>>,
) -> Result<()> {
    let player =
        connection.object_server().interface::<_, Player>("/org/mpris/MediaPlayer2").await?;
    let player_emmiter = player.signal_emitter();
    let tracklist =
        connection.object_server().interface::<_, Tracklist>("/org/mpris/MediaPlayer2").await?;
    let tracklist_emmiter = tracklist.signal_emitter();

    let no_track_path = OwnedObjectPath::try_from("/org/mpris/MediaPlayer2/TrackList/NoTrack")
        .expect("NoTrack path should always be a valid object path");

    loop {
        let ev = if let Some(ev) = rx.recv().await {
            ev
        } else {
            break;
        };

        match ev {
            Change::Volume => {
                if let Err(err) = player.get().await.volume_changed(player_emmiter).await {
                    error!(err = ?err, "Failed to emit volume changed signal");
                }
            }
            Change::PlaybackState => {
                if let Err(err) = player.get().await.playback_status_changed(player_emmiter).await {
                    error!(err = ?err, "Failed to emit playback status changed signal");
                }
            }
            Change::Metadata => {
                if let Err(err) = player.get().await.metadata_changed(player_emmiter).await {
                    error!(err = ?err, "Failed to emit metadata changed signal");
                }
            }
            Change::Queue => {
                let state = ctx.read().await;
                let tracks =
                    state.queue.iter().map(|song| song.to_mpris_id().into_inner()).collect();

                let current_track = state
                    .current_song
                    .as_ref()
                    .map_or_else(|| no_track_path.clone(), |song| song.to_mpris_id());

                if let Err(err) = Tracklist::track_list_replaced(
                    tracklist_emmiter,
                    tracks,
                    current_track.into_inner(),
                )
                .await
                {
                    error!(err = ?err, "Failed to emit track list replaced signal");
                }
            }
        }
    }

    info!("Notify consumer ended");

    Ok(())
}
