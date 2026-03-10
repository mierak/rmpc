use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::Result;
use mlua::{LuaSerdeExt, Table};
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
    lua::lualib::hooks::{ON_IDLE, ON_MESSAGE, ON_MESSAGES, ON_SONG_CHANGE, ON_STATE_CHANGE},
    mpd_ext::MpdExt,
    mpris::Change,
    song::Song,
};

static IS_PLAYING: AtomicBool = AtomicBool::new(false);

pub async fn init(
    client: Arc<AsyncClient>,
    ctx: Arc<RwLock<Ctx>>,
    mut rx: UnboundedReceiver<AppEvent>,
    tx: UnboundedSender<AppEvent>,
    mpris_tx: Option<UnboundedSender<Change>>,
    lua: mlua::Lua,
) -> Result<()> {
    if ctx.read().await.status.state == State::Play {
        start_update_loop(client.clone(), tx.clone());
    }

    let mut change_buffer = Vec::new();
    let hooks = lua.globals().get::<Table>("rmpcd")?.get::<Table>("hooks")?;

    loop {
        debug!("Waiting for events...");
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
                if ro_song != &song {
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

                    let song_hooks = hooks.get::<Table>(ON_SONG_CHANGE)?;

                    for func in song_hooks.sequence_values::<mlua::Function>() {
                        if let Err(err) = func?.call_async::<()>((&old_song, &new_song)).await {
                            error!(err = ?err, "Failed to call on_song_change callback");
                        }
                    }
                }

                if ro_status.state != new_status.state {
                    let old_state = lua.to_value(&ro_status.state)?;
                    let new_state = lua.to_value(&new_status.state)?;

                    let state_hooks = hooks.get::<Table>(ON_STATE_CHANGE)?;

                    for func in state_hooks.sequence_values::<mlua::Function>() {
                        if let Err(err) = func?.call_async::<()>((&old_state, &new_state)).await {
                            error!(err = ?err, "Failed to call on_state_change callback");
                        }
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
                        IdleEvent::Message => {
                            let messages = client.run(|c| c.read_messages()).await?;

                            let message_hooks = hooks.get::<Table>(ON_MESSAGE)?;
                            for func in message_hooks.sequence_values::<mlua::Function>() {
                                let func = func?;
                                for (k, v) in &messages.0 {
                                    if let Err(err) = func
                                        .call_async::<()>((lua.to_value(k)?, lua.to_value(v)?))
                                        .await
                                    {
                                        error!(err = ?err, "Failed to call on_message callback");
                                    }
                                }
                            }

                            let msgs_hooks = hooks.get::<Table>(ON_MESSAGES)?;
                            let messages: HashMap<String, Vec<String>> =
                                messages.0.into_iter().collect();
                            let messages = lua.to_value(&messages)?;

                            for func in msgs_hooks.sequence_values::<mlua::Function>() {
                                if let Err(err) = func?.call_async::<()>(&messages).await {
                                    error!(err = ?err, "Failed to call on_messages callback");
                                }
                            }
                        }
                        ev => {
                            debug!(?ev, "Event currently not supported");
                            // TODO receiving event without calling client::run will block the event
                            // loop forever
                            client.run(|c| c.get_current_song()).await.ok();
                        }
                    }

                    let idle_ev_hooks = hooks.get::<Table>(ON_IDLE)?;
                    for func in idle_ev_hooks.sequence_values::<mlua::Function>() {
                        if let Err(err) = func?.call_async::<()>(ev.to_string()).await {
                            error!(err = ?err, "Failed to call on_idle callback");
                        }
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
