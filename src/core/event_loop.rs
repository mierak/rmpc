use std::{
    collections::HashSet,
    io::Stdout,
    ops::Sub,
    sync::{Arc, LazyLock},
    time::Duration,
};

use crossbeam::channel::{Receiver, RecvTimeoutError};
use itertools::Itertools;
use ratatui::{
    Terminal,
    layout::Rect,
    prelude::{Backend, CrosstermBackend},
};

use super::command::{create_env, run_external};
use crate::{
    context::AppContext,
    mpd::{
        commands::{IdleEvent, State},
        mpd_client::MpdClient,
    },
    shared::{
        events::{AppEvent, WorkDone},
        ext::{duration::DurationExt, error::ErrorExt},
        id::{self, Id},
        lrc::get_lrc_path,
        macros::{status_error, status_warn},
        mpd_query::{
            EXTERNAL_COMMAND, GLOBAL_QUEUE_UPDATE, GLOBAL_STATUS_UPDATE, GLOBAL_VOLUME_UPDATE,
            MpdQueryResult, run_status_update,
        },
    },
    ui::{KeyHandleResult, Ui, UiAppEvent, UiEvent, modals::info_modal::InfoModal},
};

static ON_RESIZE_SCHEDULE_ID: LazyLock<Id> = LazyLock::new(id::new);

pub fn init(
    context: AppContext,
    event_rx: Receiver<AppEvent>,
    terminal: Terminal<CrosstermBackend<Stdout>>,
) -> std::io::Result<std::thread::JoinHandle<Terminal<CrosstermBackend<Stdout>>>> {
    std::thread::Builder::new()
        .name("main".to_owned())
        .spawn(move || main_task(context, event_rx, terminal))
}

fn main_task<B: Backend + std::io::Write>(
    mut context: AppContext,
    event_rx: Receiver<AppEvent>,
    mut terminal: Terminal<B>,
) -> Terminal<B> {
    let size = terminal.size().expect("To be able to get terminal size");
    let area = Rect::new(0, 0, size.width, size.height);
    let mut ui = Ui::new(&context).expect("UI to be created correctly");
    let event_receiver = event_rx;
    let mut render_wanted = false;
    let max_fps = f64::from(context.config.max_fps);
    let mut min_frame_duration = Duration::from_secs_f64(1f64 / max_fps);
    let mut last_render = std::time::Instant::now().sub(Duration::from_secs(10));
    let mut additional_evs = HashSet::new();
    let mut connected = true;
    ui.before_show(area, &mut context).expect("Initial render init to succeed");
    let mut _update_loop_guard = None;

    // Tmux hooks have to be initialized after ui, because ueberzugpp replaces all
    // hooks on its init instead of simply appending and might break rmpc's hooks
    let mut tmux = match crate::shared::tmux::TmuxHooks::new() {
        Ok(Some(val)) => Some(val),
        Ok(None) => None,
        Err(err) => {
            log::error!(error:? = err; "Failed to install tmux hooks");
            None
        }
    };

    // Check the playback status and start the periodic status update if needed
    if context.status.state == State::Play {
        _update_loop_guard = context
            .config
            .status_update_interval_ms
            .map(Duration::from_millis)
            .map(|interval| context.scheduler.repeated(interval, run_status_update));
    }

    loop {
        let now = std::time::Instant::now();

        let event = if render_wanted {
            match event_receiver.recv_timeout(
                min_frame_duration.checked_sub(now - last_render).unwrap_or(Duration::ZERO),
            ) {
                Ok(v) => Some(v),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => None,
            }
        } else {
            event_receiver.recv().ok()
        };

        if let Some(event) = event {
            let _lock = std::io::stdout().lock();
            match event {
                AppEvent::ConfigChanged { config: mut new_config, keep_old_theme } => {
                    // Techical limitation. Keep the old image backend because it was not rechecked
                    // anyway. Sending the escape sequences to determine image support would mess up
                    // the terminal output at this point.
                    new_config.album_art.method = context.config.album_art.method;
                    if keep_old_theme {
                        new_config.theme = context.config.theme.clone();
                    }

                    if let Err(err) = new_config.validate() {
                        status_error!(error:? = err; "Cannot change config, invalid value: '{err}'");
                        continue;
                    }

                    context.config = Arc::new(new_config);
                    let max_fps = f64::from(context.config.max_fps);
                    min_frame_duration = Duration::from_secs_f64(1f64 / max_fps);

                    if let Err(err) = ui.on_event(UiEvent::ConfigChanged, &mut context) {
                        log::error!(error:? = err; "UI failed to handle config changed event");
                        continue;
                    }

                    // Need to clear the terminal to avoid artifacts from album art and other
                    // elements
                    if let Err(err) = terminal.clear() {
                        log::error!(error:? = err; "Failed to clear terminal after config change");
                        continue;
                    }

                    render_wanted = true;
                }
                AppEvent::ThemeChanged { theme } => {
                    let mut config = context.config.as_ref().clone();
                    config.theme = theme;
                    if let Err(err) = config.validate() {
                        status_error!(error:? = err; "Cannot change theme, invalid config: '{err}'");
                        continue;
                    }
                    context.config = Arc::new(config);

                    if let Err(err) = ui.on_event(UiEvent::ConfigChanged, &mut context) {
                        log::error!(error:? = err; "UI failed to handle config changed event");
                    }

                    // Need to clear the terminal to avoid artifacts from album art and other
                    // elements
                    if let Err(err) = terminal.clear() {
                        log::error!(error:? = err; "Failed to clear terminal after config change");
                        continue;
                    }
                    render_wanted = true;
                }
                AppEvent::UserKeyInput(key) => match ui.handle_key(&mut key.into(), &mut context) {
                    Ok(KeyHandleResult::None) => continue,
                    Ok(KeyHandleResult::Quit) => {
                        if let Err(err) = ui.on_event(UiEvent::Exit, &mut context) {
                            log::error!(error:? = err, event:?; "UI failed to handle quit event");
                        }
                        break;
                    }
                    Err(err) => {
                        status_error!(err:?; "Error: {}", err.to_status());
                        render_wanted = true;
                    }
                },
                AppEvent::UserMouseInput(ev) => match ui.handle_mouse_event(ev, &mut context) {
                    Ok(()) => {}
                    Err(err) => {
                        status_error!(err:?; "Error: {}", err.to_status());
                        render_wanted = true;
                    }
                },
                AppEvent::Status(message, level) => {
                    if let Err(err) = ui.on_event(UiEvent::Status(message, level), &mut context) {
                        log::error!(error:? = err; "UI failed to handle status message event");
                    }
                    render_wanted = true;
                    // Send delayed render event to make the status message
                    // disappear
                    context.scheduler.schedule(Duration::from_secs(5), |(tx, _)| {
                        Ok(tx.send(AppEvent::RequestRender)?)
                    });
                }
                AppEvent::InfoModal { message, title, size } => {
                    if let Err(err) = ui.on_ui_app_event(
                        UiAppEvent::Modal(Box::new(
                            InfoModal::builder()
                                .context(&context)
                                .maybe_title(title)
                                .maybe_size(size)
                                .message(message)
                                .build(),
                        )),
                        &mut context,
                    ) {
                        log::error!(error:? = err; "UI failed to handle modal event");
                    }
                }
                AppEvent::Log(msg) => {
                    if let Err(err) = ui.on_event(UiEvent::LogAdded(msg), &mut context) {
                        log::error!(error:? = err; "UI failed to handle log event");
                    }
                }
                AppEvent::IdleEvent(event) => {
                    handle_idle_event(event, &context, &mut additional_evs);
                    for ev in additional_evs.drain() {
                        if let Err(err) = ui.on_event(ev, &mut context) {
                            status_error!(error:? = err, event:?; "UI failed to handle idle event, event: '{:?}', error: '{}'", event, err.to_status());
                        }
                    }
                    render_wanted = true;
                }
                AppEvent::RequestRender => {
                    render_wanted = true;
                }
                AppEvent::WorkDone(Ok(result)) => match result {
                    WorkDone::LyricsIndexed { index } => {
                        context.lrc_index = index;
                        if let Err(err) = ui.on_event(UiEvent::LyricsIndexed, &mut context) {
                            log::error!(error:? = err; "UI failed to handle lyrics indexed event");
                        }
                    }
                    WorkDone::SingleLrcIndexed { lrc_entry } => {
                        if let Some(lrc_entry) = lrc_entry {
                            context.lrc_index.add(lrc_entry);
                        }
                        if let Err(err) = ui.on_event(UiEvent::LyricsIndexed, &mut context) {
                            log::error!(error:? = err; "UI failed to handle single lyrics indexed event");
                        }
                    }
                    WorkDone::MpdCommandFinished { id, target, data } => match (id, target, data) {
                        (GLOBAL_STATUS_UPDATE, None, MpdQueryResult::Status(status)) => {
                            let current_song_id =
                                context.find_current_song_in_queue().map(|(_, song)| song.id);
                            let current_status = context.status.state;
                            let current_updating_db = context.status.updating_db;
                            context.status = status;
                            let mut song_changed = false;

                            match (current_updating_db, context.status.updating_db) {
                                (None, Some(_)) => {
                                    // update of db started
                                    context.db_update_start = Some(std::time::Instant::now());
                                }
                                (Some(_), None) => {
                                    // update of db ended
                                    context.db_update_start = None;
                                }
                                (Some(_), Some(_)) => {
                                    // rmpc is opened after db being updated
                                    // beforehand -> reassign
                                    if context.db_update_start.is_none() {
                                        context.db_update_start = Some(std::time::Instant::now());
                                    }
                                }
                                _ => {}
                            }

                            match context.status.state {
                                State::Play => {
                                    if current_status != context.status.state {
                                        _update_loop_guard = context
                                            .config
                                            .status_update_interval_ms
                                            .map(Duration::from_millis)
                                            .map(|interval| {
                                                context
                                                    .scheduler
                                                    .repeated(interval, run_status_update)
                                            });
                                    }
                                }
                                State::Pause => {
                                    _update_loop_guard = None;
                                }
                                State::Stop => {
                                    song_changed = true;
                                    _update_loop_guard = None;
                                }
                            }

                            if let Some((_, song)) = context.find_current_song_in_queue() {
                                if Some(song.id) != current_song_id {
                                    if let Some(command) = &context.config.on_song_change {
                                        let lrc_path = context
                                            .config
                                            .lyrics_dir
                                            .as_ref()
                                            .and_then(|dir| get_lrc_path(dir, &song.file).ok())
                                            .map(|path| path.to_string_lossy().into_owned())
                                            .unwrap_or_default();
                                        let lrc = context.find_lrc().ok().flatten();
                                        let pid = std::process::id();
                                        let duration = song
                                            .duration
                                            .map_or_else(String::new, |d| d.to_string());

                                        let mut env = song
                                            .clone()
                                            .metadata
                                            .into_iter()
                                            .map(|(mut k, mut v)| {
                                                k.make_ascii_uppercase();
                                                (k, std::mem::take(v.last_mut()))
                                            })
                                            .collect_vec();

                                        env.push(("FILE".to_owned(), song.file.clone()));
                                        env.push(("DURATION".to_owned(), duration));
                                        env.push(("PID".to_owned(), pid.to_string()));
                                        env.push(("HAS_LRC".to_owned(), lrc.is_some().to_string()));
                                        env.push(("LRC_FILE".to_owned(), lrc_path));
                                        env.push((
                                            "VERSION".to_owned(),
                                            env!("CARGO_PKG_VERSION").to_string(),
                                        ));
                                        run_external(command.clone(), env);
                                    }
                                    song_changed = true;
                                }
                            }
                            if song_changed {
                                if let Err(err) = ui.on_event(UiEvent::SongChanged, &mut context) {
                                    status_error!(error:? = err; "UI failed to handle idle event, error: '{}'", err.to_status());
                                }
                            }
                            render_wanted = true;
                        }
                        ("global_volume_update", None, MpdQueryResult::Volume(volume)) => {
                            context.status.volume = volume;
                            render_wanted = true;
                        }
                        ("global_queue_update", None, MpdQueryResult::Queue(queue)) => {
                            context.queue = queue.unwrap_or_default();
                            render_wanted = true;
                        }
                        (
                            EXTERNAL_COMMAND,
                            None,
                            MpdQueryResult::ExternalCommand(command, songs),
                        ) => {
                            let songs = songs.iter().map(|s| s.file.as_str());
                            run_external(command, create_env(&context, songs));
                        }
                        (id, target, data) => {
                            if let Err(err) = ui.on_command_finished(id, target, data, &mut context)
                            {
                                log::error!(error:? = err; "UI failed to handle command finished event");
                            }
                        }
                    },
                    WorkDone::None => {}
                },
                AppEvent::WorkDone(Err(err)) => {
                    status_error!("{}", err);
                }
                AppEvent::Resized { columns, rows } => {
                    context.scheduler.schedule_replace(
                        *ON_RESIZE_SCHEDULE_ID,
                        Duration::from_millis(500),
                        move |(tx, _)| {
                            tx.send(AppEvent::ResizedDebounced { columns, rows })?;
                            Ok(())
                        },
                    );
                    render_wanted = true;
                }
                AppEvent::ResizedDebounced { columns, rows } => {
                    if let Err(err) = ui.resize(Rect::new(0, 0, columns, rows), &context) {
                        log::error!(error:? = err, event:?; "UI failed to handle resize event");
                    }

                    if let Some(cmd) = &context.config.on_resize {
                        let cmd = Arc::clone(cmd);
                        let mut env = create_env(&context, std::iter::empty::<&str>());
                        env.push(("COLS", columns.to_string()));
                        env.push(("ROWS", rows.to_string()));
                        log::debug!("Executing on resize");
                        run_external(cmd, env);
                    }
                }
                AppEvent::UiEvent(event) => match ui.on_ui_app_event(event, &mut context) {
                    Ok(()) => {}
                    Err(err) => {
                        status_error!(err:?; "Error: {}", err.to_status());
                        render_wanted = true;
                    }
                },
                AppEvent::Reconnected => {
                    for ev in [IdleEvent::Player, IdleEvent::Playlist, IdleEvent::Options] {
                        handle_idle_event(ev, &context, &mut additional_evs);
                    }
                    if let Err(err) = ui.on_event(UiEvent::Reconnected, &mut context) {
                        log::error!(error:? = err, event:?; "UI failed to handle resize event");
                    }
                    status_warn!("rmpc reconnected to MPD and will reinitialize");
                    connected = true;
                }
                AppEvent::LostConnection => {
                    if context.status.state != State::Stop {
                        _update_loop_guard = None;
                        context.status.state = State::Stop;
                    }
                    if connected {
                        status_error!("rmpc lost connection to MPD and will try to reconnect");
                    }
                    connected = false;
                }
                AppEvent::TmuxHook { hook } => {
                    if let Some(tmux) = &mut tmux {
                        let old_visible = tmux.visible;
                        if let Err(err) = tmux.update_visible() {
                            log::error!(err:?, hook:?; "Failed to update tmux visibility");
                            continue;
                        }

                        let event = match (tmux.visible, old_visible) {
                            (true, false) => UiEvent::Displayed,
                            (false, true) => UiEvent::Hidden,
                            _ => continue,
                        };

                        match ui.on_event(event, &mut context) {
                            Ok(()) => {}
                            Err(err) => {
                                status_error!(err:?; "Error: {}", err.to_status());
                                render_wanted = true;
                            }
                        }
                    }
                }
            }
        }
        if render_wanted {
            let till_next_frame =
                min_frame_duration.saturating_sub(now.duration_since(last_render));
            if till_next_frame != Duration::ZERO {
                continue;
            }
            terminal
                .draw(|frame| {
                    if let Err(err) = ui.render(frame, &mut context) {
                        log::error!(error:? = err; "Failed to render a frame");
                    }
                })
                .expect("Expected render to succeed");

            context.finish_frame();
            last_render = now;
            render_wanted = false;
        }
    }

    terminal
}

fn handle_idle_event(event: IdleEvent, context: &AppContext, result_ui_evs: &mut HashSet<UiEvent>) {
    match event {
        IdleEvent::Mixer if context.supported_commands.contains("getvol") => {
            context
                .query()
                .id(GLOBAL_VOLUME_UPDATE)
                .replace_id("volume")
                .query(move |client| Ok(MpdQueryResult::Volume(client.get_volume()?)));
        }
        IdleEvent::Mixer => {
            context
                .query()
                .id(GLOBAL_STATUS_UPDATE)
                .replace_id("status")
                .query(move |client| Ok(MpdQueryResult::Status(client.get_status()?)));
        }
        IdleEvent::Options => {
            context
                .query()
                .id(GLOBAL_STATUS_UPDATE)
                .replace_id("status")
                .query(move |client| Ok(MpdQueryResult::Status(client.get_status()?)));
        }
        IdleEvent::Player => {
            context
                .query()
                .id(GLOBAL_STATUS_UPDATE)
                .replace_id("status")
                .query(move |client| Ok(MpdQueryResult::Status(client.get_status()?)));
        }
        IdleEvent::Playlist | IdleEvent::Sticker => {
            let fetch_stickers = context.should_fetch_stickers;
            context.query().id(GLOBAL_QUEUE_UPDATE).replace_id("playlist").query(move |client| {
                Ok(MpdQueryResult::Queue(client.playlist_info(fetch_stickers)?))
            });
        }
        IdleEvent::StoredPlaylist => {}
        IdleEvent::Database => {
            context
                .query()
                .id(GLOBAL_STATUS_UPDATE)
                .replace_id("status")
                .query(move |client| Ok(MpdQueryResult::Status(client.get_status()?)));
        }
        IdleEvent::Update => {}
        IdleEvent::Output
        | IdleEvent::Partition
        | IdleEvent::Subscription
        | IdleEvent::Message
        | IdleEvent::Neighbor
        | IdleEvent::Mount => {
            log::warn!(event:?; "Received unhandled event");
        }
    }

    if let Ok(ev) = event.try_into() {
        result_ui_evs.insert(ev);
    }
}
