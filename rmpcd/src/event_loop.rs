use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use mlua::LuaSerdeExt;
use rmpc_mpd::{
    commands::{IdleEvent, State},
    mpd_client::{AlbumArtOrder, MpdClient},
};
use tokio::sync::{
    RwLock,
    mpsc::{UnboundedReceiver, UnboundedSender},
};
use tracing::{debug, error, warn};

use crate::{
    AppEvent,
    async_client::AsyncClient,
    ctx::Ctx,
    ext::SenderExt,
    mpd_ext::MpdExt,
    mpris::Change,
    song::Song,
};

static IS_PLAYING: AtomicBool = AtomicBool::new(false);

#[allow(clippy::too_many_arguments)]
pub async fn init(
    client: Arc<AsyncClient>,
    ctx: Arc<RwLock<Ctx>>,
    mut rx: UnboundedReceiver<AppEvent>,
    tx: UnboundedSender<AppEvent>,
    mpris_tx: Option<UnboundedSender<Change>>,
    lua: mlua::Lua,
    on_change: Option<mlua::Function>,
    on_state_change: Option<mlua::Function>,
) -> Result<()> {
    if ctx.read().await.status.state == State::Play {
        start_update_loop(client.clone(), tx.clone());
    }

    let mut change_buffer = Vec::new();
    loop {
        let Some(ev) = rx.recv().await else {
            warn!("Idle task ended");
            break;
        };

        match ev {
            AppEvent::StatusUpdate(new_status) => {
                let song = client.run(|c| c.get_current_song()).await?;
                let mut album_art_changed = false;
                let mut album_art = None;

                let ro_mpd_state = ctx.read().await;
                let ro_status = &ro_mpd_state.status;
                let ro_song = &ro_mpd_state.current_song;
                if let Some(on_change) = &on_change
                    && ro_song != &song
                {
                    let old_song = lua.to_value(&ro_song.as_ref().map(Song::from))?;
                    let new_song = lua.to_value(&song.as_ref().map(Song::from))?;

                    album_art = if let Some(s) = &song {
                        let uri = s.file.clone();
                        album_art_changed = true;
                        client
                            .run(move |c| c.find_album_art(&uri, AlbumArtOrder::EmbeddedFirst))
                            .await?
                    } else {
                        None
                    };

                    if let Err(err) = on_change.call::<()>((old_song, new_song)) {
                        error!(err = ?err, "Failed to call on_change callback");
                    }
                }

                if ro_status.state != new_status.state
                    && let Some(on_state_change) = &on_state_change
                {
                    let old_state = lua.to_value(&ro_status.state)?;
                    let new_state = lua.to_value(&new_status.state)?;
                    if let Err(err) = on_state_change.call::<()>((old_state, new_state)) {
                        error!(err = ?err, "Failed to call on_state_change callback");
                    }
                }

                change_buffer.push(Change::Metadata); // TODO
                if ro_status.state != new_status.state {
                    change_buffer.push(Change::PlaybackState);
                    match new_status.state {
                        State::Play => {
                            let tx = tx.clone();
                            let client = client.clone();
                            start_update_loop(client, tx);
                        }
                        State::Pause | State::Stop => {
                            IS_PLAYING.store(false, Ordering::Relaxed);
                        }
                    }
                }

                drop(ro_mpd_state);
                let mut state = ctx.write().await;
                state.status = new_status;
                state.current_song = song;
                if album_art_changed {
                    state.album_art = album_art;
                }

                for change in change_buffer.drain(..) {
                    if let Some(tx) = &mpris_tx {
                        tx.send_safe(change);
                    }
                }
            }
            AppEvent::Idle(events) => {
                for ev in events {
                    match ev {
                        IdleEvent::Player => {
                            let new_status = client.run(|c| c.get_status()).await?;
                            tx.send_safe(AppEvent::StatusUpdate(new_status));
                        }
                        IdleEvent::Mixer => {
                            let new_status = client.run(|c| c.get_status()).await?;
                            let old_volume = ctx.read().await.status.volume;
                            ctx.write().await.status.volume = new_status.volume;

                            if old_volume != new_status.volume
                                && let Some(tx) = &mpris_tx
                            {
                                tx.send_safe(Change::Volume);
                            }
                        }
                        IdleEvent::Playlist => {
                            let new_queue = client.run(|c| c.playlist_info()).await?;
                            ctx.write().await.queue = new_queue.unwrap_or_default();
                            if let Some(tx) = &mpris_tx {
                                tx.send_safe(Change::Queue);
                            }
                        }
                        IdleEvent::Options => {}
                        IdleEvent::Database => {}
                        IdleEvent::Update => {}
                        IdleEvent::StoredPlaylist => {}
                        IdleEvent::Output => {}
                        IdleEvent::Partition => {}
                        IdleEvent::Sticker => {}
                        IdleEvent::Subscription => {}
                        IdleEvent::Message => {}
                        IdleEvent::Neighbor => {}
                        IdleEvent::Mount => {}
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn start_update_loop(client: Arc<AsyncClient>, tx: UnboundedSender<AppEvent>) {
    IS_PLAYING.store(true, Ordering::Relaxed);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            if !IS_PLAYING.load(Ordering::Relaxed) {
                break;
            }
            debug!("Tick: checking status...");

            match client.run(|c| c.get_status()).await {
                Ok(s) => tx.send_safe(AppEvent::StatusUpdate(s.clone())),
                Err(err) => {
                    error!(err = ?err, "Failed to get status in tick");
                    break;
                }
            }
        }
    });
}
