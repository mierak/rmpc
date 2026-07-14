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
    LoopStatus,
    Shuffle,
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

    macro_rules! emit_player_changed {
        ($method:ident, $property:literal) => {
            if let Err(err) = player.get().await.$method(player_emmiter).await {
                error!(err = ?err, property = $property, "Failed to emit property changed signal");
            }
        };
    }

    loop {
        let ev = if let Some(ev) = rx.recv().await {
            ev
        } else {
            break;
        };

        match ev {
            Change::Volume => {
                emit_player_changed!(volume_changed, "Volume");
            }
            Change::PlaybackState => {
                emit_player_changed!(playback_status_changed, "PlaybackStatus");
                emit_player_changed!(can_play_changed, "CanPlay");
                emit_player_changed!(can_go_next_changed, "CanGoNext");
                emit_player_changed!(can_go_previous_changed, "CanGoPrevious");
            }
            Change::LoopStatus => {
                emit_player_changed!(loop_status_changed, "LoopStatus");
                emit_player_changed!(can_go_next_changed, "CanGoNext");
                emit_player_changed!(can_go_previous_changed, "CanGoPrevious");
            }
            Change::Shuffle => {
                emit_player_changed!(shuffle_changed, "Shuffle");
            }
            Change::Metadata => {
                emit_player_changed!(metadata_changed, "Metadata");
                emit_player_changed!(can_go_next_changed, "CanGoNext");
                emit_player_changed!(can_go_previous_changed, "CanGoPrevious");
            }
            Change::Queue => {
                let (tracks, current_track) = {
                    let state = ctx.read().await;
                    let tracks: Vec<_> =
                        state.queue.iter().map(|song| song.to_mpris_id().into_inner()).collect();
                    let current_track = state
                        .current_song
                        .as_ref()
                        .map_or_else(|| no_track_path.clone(), |song| song.to_mpris_id());
                    (tracks, current_track)
                };

                if let Err(err) = Tracklist::track_list_replaced(
                    tracklist_emmiter,
                    tracks,
                    current_track.into_inner(),
                )
                .await
                {
                    error!(err = ?err, "Failed to emit track list replaced signal");
                }

                emit_player_changed!(can_play_changed, "CanPlay");
                emit_player_changed!(can_go_next_changed, "CanGoNext");
                emit_player_changed!(can_go_previous_changed, "CanGoPrevious");
            }
        }
    }

    info!("Notify consumer ended");

    Ok(())
}
