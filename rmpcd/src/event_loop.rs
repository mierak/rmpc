use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use rmpc_mpd::{
    commands::{IdleEvent, State},
    mpd_client::{AlbumArtOrder, MpdClient},
};
use tokio::{
    select,
    sync::{
        RwLock,
        mpsc::{UnboundedReceiver, UnboundedSender},
    },
};
use tracing::{error, info, trace, warn};

use crate::{
    AppEvent,
    async_client::AsyncClient,
    ctx::Ctx,
    ext::SenderExt,
    lua::{
        lualib::mpd::types::Song,
        plugin::{self, LuaPlugin, PluginStore, PluginsEvent},
    },
    mpd_ext::MpdExt,
    mpris::Change,
};

static IS_PLAYING: AtomicBool = AtomicBool::new(false);

pub async fn init(
    client: Arc<AsyncClient>,
    ctx: Arc<RwLock<Ctx>>,
    mut app_ev_rx: UnboundedReceiver<AppEvent>,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
    app_ev_tx: UnboundedSender<AppEvent>,
    mpris_tx: Option<UnboundedSender<Change>>,
    plugin_store: PluginStore<LuaPlugin>,
) -> Result<()> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<PluginsEvent>();

    let plugin_handle =
        tokio::spawn(async move { plugin::init_plugin_loop(rx, plugin_store).await });

    if ctx.read().await.status.state == State::Play {
        start_update_loop(client.clone(), app_ev_tx.clone());
    }

    let mut change_buffer = Vec::new();

    loop {
        trace!("Waiting for events...");
        let ev = select! {
            ev = app_ev_rx.recv() => {
                if let Some(ev) = ev { ev } else {
                    warn!("Idle task ended because app event channel was closed");
                    break;
                }
            },
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received, stopping event loop");
                tx.send_safe(PluginsEvent::Shutdown);
                break;
            }
        };
        trace!(?ev, "Received event");

        match ev {
            AppEvent::StatusUpdate(new_status) => {
                let song = client.run(|c| c.get_current_song()).await?;
                let mut album_art_changed = false;
                let mut album_art = None;

                let ro_mpd_state = ctx.read().await;
                let ro_status = &ro_mpd_state.status;
                let ro_song = &ro_mpd_state.current_song;
                if ro_song != &song {
                    album_art = if let Some(s) = &song {
                        let uri = s.file.clone();
                        album_art_changed = true;
                        client
                            .run(move |c| c.find_album_art(&uri, AlbumArtOrder::EmbeddedFirst))
                            .await?
                    } else {
                        None
                    };

                    tx.send_safe(PluginsEvent::SongChange {
                        old: ro_song.as_ref().map(Song::from),
                        new: song.as_ref().map(Song::from),
                    });
                }

                if ro_status.state != new_status.state {
                    tx.send_safe(PluginsEvent::StateChange {
                        old: ro_status.clone().into(),
                        new: new_status.clone().into(),
                    });
                }

                change_buffer.push(Change::Metadata); // TODO
                if ro_status.state != new_status.state {
                    change_buffer.push(Change::PlaybackState);
                    match new_status.state {
                        State::Play => {
                            let tx = app_ev_tx.clone();
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
                            app_ev_tx.send_safe(AppEvent::StatusUpdate(new_status));
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
                        IdleEvent::Message => {
                            let messages =
                                client.run(|c| c.read_messages()).await?.0.into_iter().collect();
                            tx.send_safe(PluginsEvent::Messages { messages });
                        }
                        ev => {
                            trace!(?ev, "Event currently not supported");
                            client.skip_to_idle().await;
                        }
                    }

                    tx.send_safe(PluginsEvent::Idle { event: ev });
                }
            }
            AppEvent::Reconnected => {
                tx.send_safe(PluginsEvent::Reconnect);
            }
        }
    }

    trace!("Waiting for plugin loop to finish...");

    plugin_handle.await??;

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
            trace!("Tick: checking status...");

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
