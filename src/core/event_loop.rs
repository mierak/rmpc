use std::{collections::HashSet, io::Stdout, ops::Sub, time::Duration};

use crossbeam::channel::{Receiver, RecvTimeoutError};
use itertools::Itertools;
use ratatui::{
    layout::Rect,
    prelude::{Backend, CrosstermBackend},
    Terminal,
};

use crate::{
    context::AppContext,
    mpd::{
        commands::{IdleEvent, State},
        mpd_client::MpdClient,
    },
    shared::{
        events::{AppEvent, WorkDone},
        ext::{duration::DurationExt, error::ErrorExt},
        macros::{status_error, status_warn, try_skip},
        mpd_query::MpdQueryResult,
    },
    ui::{KeyHandleResult, Ui, UiEvent},
};

use super::{
    command::{create_env, run_external},
    update_loop::UpdateLoop,
};

pub const EXTERNAL_COMMAND: &str = "external_command";
pub const GLOBAL_STATUS_UPDATE: &str = "global_status_update";
pub const GLOBAL_VOLUME_UPDATE: &str = "global_volume_update";
pub const GLOBAL_QUEUE_UPDATE: &str = "global_queue_update";

pub fn init(
    context: AppContext,
    event_rx: Receiver<AppEvent>,
    update_loop: UpdateLoop,
    terminal: Terminal<CrosstermBackend<Stdout>>,
) -> std::io::Result<std::thread::JoinHandle<Terminal<CrosstermBackend<Stdout>>>> {
    std::thread::Builder::new()
        .name("main".to_owned())
        .spawn(move || main_task(context, event_rx, update_loop, terminal))
}

fn main_task<B: Backend + std::io::Write>(
    mut context: AppContext,
    event_rx: Receiver<AppEvent>,
    mut render_loop: UpdateLoop,
    mut terminal: Terminal<B>,
) -> Terminal<B> {
    let size = terminal.size().expect("To be able to get terminal size");
    let area = Rect::new(0, 0, size.width, size.height);
    let mut ui = Ui::new(&context).expect("UI to be created correctly");
    let event_receiver = event_rx;
    let mut render_wanted = false;
    let max_fps = 30f64;
    let min_frame_duration = Duration::from_secs_f64(1f64 / max_fps);
    let mut last_render = std::time::Instant::now().sub(Duration::from_secs(10));
    let mut additional_evs = HashSet::new();
    let mut connected = true;
    ui.before_show(area, &mut context)
        .expect("Initial render init to succeed");

    loop {
        let now = std::time::Instant::now();

        let event = if render_wanted {
            match event_receiver.recv_timeout(
                min_frame_duration
                    .checked_sub(now - last_render)
                    .unwrap_or(Duration::ZERO),
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
                AppEvent::UserKeyInput(key) => match ui.handle_key(&mut key.into(), &mut context) {
                    Ok(KeyHandleResult::None) => continue,
                    Ok(KeyHandleResult::Quit) => {
                        if let Err(err) = ui.on_event(UiEvent::Exit, &context) {
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
                    if let Err(err) = ui.on_event(UiEvent::Status(message, level), &context) {
                        log::error!(error:? = err; "UI failed to handle status message event");
                    }
                }
                AppEvent::Log(msg) => {
                    if let Err(err) = ui.on_event(UiEvent::LogAdded(msg), &context) {
                        log::error!(error:? = err; "UI failed to handle log event");
                    }
                }
                AppEvent::IdleEvent(event) => {
                    handle_idle_event(event, &context, &mut additional_evs);
                    for ev in additional_evs.drain() {
                        if let Err(err) = ui.on_event(ev, &context) {
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
                        if let Err(err) = ui.on_event(UiEvent::LyricsIndexed, &context) {
                            log::error!(error:? = err; "UI failed to lyrics indexed event");
                        }
                    }
                    WorkDone::MpdCommandFinished { id, target, data } => match (id, target, data) {
                        (GLOBAL_STATUS_UPDATE, None, MpdQueryResult::Status(status)) => {
                            let current_song_id = context.find_current_song_in_queue().map(|(_, song)| song.id);
                            let current_status = context.status.state;
                            context.status = status;
                            let mut song_changed = false;

                            match context.status.state {
                                State::Play => {
                                    if current_status != context.status.state {
                                        try_skip!(render_loop.start(), "Failed to start render loop");
                                    }
                                }
                                State::Pause => {
                                    if current_status != context.status.state {
                                        try_skip!(render_loop.stop(), "Failed to stop render loop");
                                    }
                                }
                                State::Stop => {
                                    song_changed = true;
                                    if current_status != context.status.state {
                                        try_skip!(render_loop.stop(), "Failed to stop render loop");
                                    }
                                }
                            }

                            if let Some((_, song)) = context.find_current_song_in_queue() {
                                if Some(song.id) != current_song_id {
                                    if let Some(command) = context.config.on_song_change {
                                        let env = song
                                            .clone()
                                            .metadata
                                            .into_iter()
                                            .map(|(mut k, v)| {
                                                k.make_ascii_uppercase();
                                                (k, v)
                                            })
                                            .chain(std::iter::once(("FILE".to_owned(), song.file.clone())))
                                            .chain(std::iter::once((
                                                "DURATION".to_owned(),
                                                song.duration.map_or_else(String::new, |d| d.to_string()),
                                            )))
                                            .collect_vec();
                                        run_external(command, env);
                                    }
                                    song_changed = true;
                                }
                            }
                            if song_changed {
                                if let Err(err) = ui.on_event(UiEvent::SongChanged, &context) {
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
                        (EXTERNAL_COMMAND, None, MpdQueryResult::ExternalCommand(command, songs)) => {
                            let songs = songs.iter().map(|s| s.file.as_str());
                            run_external(command, create_env(&context, songs));
                        }
                        (id, target, data) => {
                            if let Err(err) = ui.on_command_finished(id, target, data, &mut context) {
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
                    if let Err(err) = ui.resize(Rect::new(0, 0, columns, rows), &context) {
                        log::error!(error:? = err, event:?; "UI failed to handle resize event");
                    }
                    render_wanted = true;
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
                    if let Err(err) = ui.on_event(UiEvent::Reconnected, &context) {
                        log::error!(error:? = err, event:?; "UI failed to handle resize event");
                    }
                    status_warn!("rmpc reconnected to MPD and will reinitialize");
                    connected = true;
                }
                AppEvent::LostConnection => {
                    if context.status.state != State::Stop {
                        try_skip!(render_loop.stop(), "Failed to stop render loop");
                        context.status.state = State::Stop;
                    }
                    if connected {
                        status_error!("rmpc lost connection to MPD and will try to reconnect");
                    }
                    connected = false;
                }
            }
        }
        if render_wanted {
            let till_next_frame = min_frame_duration.saturating_sub(now.duration_since(last_render));
            if till_next_frame != Duration::ZERO {
                continue;
            }
            terminal
                .draw(|frame| {
                    if let Err(err) = ui.render(frame, &mut context) {
                        log::error!(error:? = err; "Failed to render a frame");
                    };
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
            context
                .query()
                .id(GLOBAL_QUEUE_UPDATE)
                .replace_id("playlist")
                .query(move |client| Ok(MpdQueryResult::Queue(client.playlist_info(fetch_stickers)?)));
        }
        IdleEvent::StoredPlaylist => {}
        IdleEvent::Database => {}
        IdleEvent::Update => {}
        IdleEvent::Output
        | IdleEvent::Partition
        | IdleEvent::Subscription
        | IdleEvent::Message
        | IdleEvent::Neighbor
        | IdleEvent::Mount => {
            log::warn!(event:?; "Received unhandled event");
        }
    };

    if let Ok(ev) = event.try_into() {
        result_ui_evs.insert(ev);
    }
}
