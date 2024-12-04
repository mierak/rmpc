#![deny(clippy::unwrap_used, clippy::pedantic)]
#![allow(
    clippy::single_match,
    clippy::type_complexity,
    clippy::module_name_repetitions,
    clippy::unused_self,
    clippy::unnested_or_patterns,
    clippy::match_same_arms,
    clippy::manual_let_else,
    clippy::needless_return,
    clippy::zero_sized_map_values,
    clippy::too_many_lines,
    clippy::match_single_binding,
    clippy::struct_field_names,
    clippy::redundant_closure_for_method_calls,
    unused_macros
)]
use std::{
    collections::{HashSet, VecDeque},
    io::{Read, Write},
    ops::Sub,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread::Builder,
    time::Duration,
};

use anyhow::{Context, Result};
use clap::Parser;
use cli::{create_env, run_external};
use config::{
    cli::{Args, Command},
    tabs::PaneType,
    ConfigFile,
};
use context::AppContext;
use crossbeam::channel::{bounded, unbounded, Receiver, RecvTimeoutError, Sender, TryRecvError};
use crossterm::event::{Event, KeyEvent};
use itertools::Itertools;
use log::{error, info, warn};
use mpd::{
    client::Client,
    commands::{idle::IdleEvent, Decoder, Output, Song, State, Status, Volume},
};
use ratatui::{prelude::Backend, widgets::ListItem, Terminal};
use rustix::path::Arg;
use shared::{
    dependencies::{DEPENDENCIES, FFMPEG, FFPROBE, PYTHON3, PYTHON3MUTAGEN, UEBERZUGPP, YTDLP},
    lrc::LrcIndex,
};
use shared::{
    env::ENV,
    ext::{duration::DurationExt, error::ErrorExt},
    logging,
    macros::{status_error, try_cont, try_skip},
    mouse_event::{MouseEvent, MouseEventTracker},
    tmux,
    ytdlp::YtDlp,
};
use ui::{panes::browser::DirOrSong, Level, UiAppEvent, UiEvent};

use crate::{
    config::Config,
    mpd::mpd_client::MpdClient,
    shared::macros::{status_warn, try_ret},
    ui::Ui,
};

#[cfg(test)]
mod tests {
    pub mod fixtures;
}

mod cli;
mod config;
mod context;
mod mpd;
mod shared;
mod ui;

#[derive(derive_more::Debug)]
pub struct MpdQuery {
    id: &'static str,
    target: Option<PaneType>,
    #[debug(skip)]
    callback: Box<dyn FnOnce(&mut Client<'_>) -> Result<MpdQueryResult> + Send>,
}

#[derive(derive_more::Debug)]
pub struct MpdCommand2 {
    #[debug(skip)]
    callback: Box<dyn FnOnce(&mut Client<'_>) -> Result<()> + Send>,
}

#[derive(Debug)]
pub enum WorkRequest {
    DownloadYoutube { url: String },
    IndexLyrics { lyrics_dir: &'static str },
    MpdQuery(MpdQuery),
    MpdCommand(MpdCommand2),
    Command(Command),
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // the instances are short lived events, its fine.
pub enum WorkDone {
    LyricsIndexed {
        index: LrcIndex,
    },
    MpdCommandFinished {
        id: &'static str,
        target: Option<PaneType>,
        data: MpdQueryResult,
    },
    None,
}

#[derive(Debug)]
pub enum MpdQueryResult {
    Preview(Option<Vec<ListItem<'static>>>),
    SongsList(Vec<Song>),
    DirOrSong(Vec<DirOrSong>),
    LsInfo(Vec<String>),
    AddToPlaylist { playlists: Vec<String>, song_file: String },
    AlbumArt(Option<Vec<u8>>),
    Status(Status),
    Queue(Option<Vec<Song>>),
    Volume(Volume),
    Outputs(Vec<Output>),
    Decoders(Vec<Decoder>),
    ExternalCommand(&'static [&'static str], Vec<Song>),
}

#[derive(Debug)]
pub enum AppEvent {
    UserKeyInput(KeyEvent),
    UserMouseInput(MouseEvent),
    Status(String, Level),
    Log(Vec<u8>),
    IdleEvent(IdleEvent),
    RequestRender(bool),
    Resized { columns: u16, rows: u16 },
    WorkDone(Result<WorkDone>),
    UiAppEvent(UiAppEvent),
}

fn main() -> Result<()> {
    let mut args = Args::parse();
    match args.command {
        Some(Command::Config { current: false }) => {
            std::io::stdout().write_all(include_bytes!("../assets/example_config.ron"))?;
        }
        Some(Command::Theme { current: false }) => {
            std::io::stdout().write_all(include_bytes!("../assets/example_theme.ron"))?;
        }
        Some(Command::Config { current: true }) => {
            let mut file = std::fs::File::open(&args.config)
                .with_context(|| format!("Config file was not found at '{}'", args.config.to_string_lossy()))?;
            let mut config = String::new();
            file.read_to_string(&mut config)?;
            println!("{config}");
        }
        Some(Command::Theme { current: true }) => {
            let config_file = ConfigFile::read(&args.config)
                .with_context(|| format!("Config file was not found at '{}'", args.config.to_string_lossy()))?;
            let config_dir = args
                .config
                .parent()
                .with_context(|| format!("Invalid config path '{}'", args.config.to_string_lossy()))?;
            let theme_path = config_file
                .theme_path(config_dir)
                .context("No theme file specified in the config. Default theme is used.")?;
            let mut file = std::fs::File::open(&theme_path)
                .with_context(|| format!("Theme file was not found at '{}'", theme_path.to_string_lossy()))?;
            let mut theme = String::new();
            file.read_to_string(&mut theme)?;
            println!("{theme}");
        }
        Some(Command::DebugInfo) => {
            let config_file = ConfigFile::read(&args.config).unwrap_or_default();
            let config = config_file.clone().into_config(
                Some(&args.config),
                std::mem::take(&mut args.address),
                std::mem::take(&mut args.password),
                false,
            )?;
            let mut mpd_host = ENV.var("MPD_HOST").unwrap_or_else(|_| "unset".to_string());
            if let Some(at_idx) = mpd_host.find('@') {
                mpd_host.replace_range(..at_idx, "***");
            }
            let mpd_port = ENV.var("MPD_PORT").unwrap_or_else(|_| "unset".to_string());

            println!(
                "rmpc {}{}",
                env!("CARGO_PKG_VERSION"),
                option_env!("VERGEN_GIT_DESCRIBE")
                    .map(|g| format!(" git {g}"))
                    .unwrap_or_default()
            );
            println!("\n{:<20} {}", "Config path", args.config.as_str()?);
            println!("{:<20} {:?}", "Theme path", config_file.theme);

            println!("\nMPD:");
            println!("{:<20} {:?}", "Address", config_file.address);
            println!("{:<20} {:?}", "Resolved Address", config.address);
            println!("{:<20} {mpd_host}", "MPD_HOST");
            println!("{:<20} {mpd_port}", "MPD_PORT");

            println!("\nYoutube playback:");
            println!("{:<20} {:?}", "Cache dir", config.cache_dir);
            println!("{}", FFMPEG.display());
            println!("{}", FFPROBE.display());
            println!("{}", YTDLP.display());
            println!("{}", PYTHON3.display());
            println!("{}", PYTHON3MUTAGEN.display());

            println!("\nImage protocol:");
            println!("{:<20} {}", "Requested", config_file.album_art.method);
            println!("{:<20} {}", "Resolved", config.album_art.method);
            println!("{:<20} {}", "TMUX", tmux::is_inside_tmux());
            println!("{}", UEBERZUGPP.display());
        }
        Some(Command::Version) => {
            println!(
                "rmpc {}{}",
                env!("CARGO_PKG_VERSION"),
                option_env!("VERGEN_GIT_DESCRIBE")
                    .map(|g| format!(" git {g}"))
                    .unwrap_or_default()
            );
        }
        Some(cmd) => {
            logging::init_console().expect("Logger to initialize");
            let config: &'static Config = Box::leak(Box::new(match ConfigFile::read(&args.config) {
                Ok(val) => val.into_config(
                    Some(&args.config),
                    std::mem::take(&mut args.address),
                    std::mem::take(&mut args.password),
                    true,
                )?,
                Err(_err) => ConfigFile::default().into_config(
                    None,
                    std::mem::take(&mut args.address),
                    std::mem::take(&mut args.password),
                    true,
                )?,
            }));
            let mut client = Client::init(config.address, config.password, "", true)?;
            cmd.execute(&mut client, config)?;
        }
        None => {
            let (tx, rx) = unbounded::<AppEvent>();
            logging::init(tx.clone()).expect("Logger to initialize");
            log::debug!(rev = env!("VERGEN_GIT_DESCRIBE"); "rmpc started");
            std::thread::Builder::new()
                .name("dependency_check".to_string())
                .spawn(|| DEPENDENCIES.iter().for_each(|d| d.log()))?;

            let (worker_tx, worker_rx) = unbounded::<WorkRequest>();

            let config = match ConfigFile::read(&args.config) {
                Ok(val) => val.into_config(
                    Some(&args.config),
                    std::mem::take(&mut args.address),
                    std::mem::take(&mut args.password),
                    false,
                )?,
                Err(err) => {
                    status_warn!(err:?; "Failed to read config. Using default values. Check logs for more information");
                    ConfigFile::default().into_config(
                        None,
                        std::mem::take(&mut args.address),
                        std::mem::take(&mut args.password),
                        false,
                    )?
                }
            };

            if let Some(lyrics_dir) = config.lyrics_dir {
                try_ret!(
                    worker_tx.send(WorkRequest::IndexLyrics { lyrics_dir }),
                    "Failed to request lyrics indexing"
                );
            }
            try_ret!(tx.send(AppEvent::RequestRender(false)), "Failed to render first frame");

            let mut client = try_ret!(
                Client::init(config.address, config.password, "command", false),
                "Failed to connect to MPD"
            );
            client.set_read_timeout(None)?;

            let terminal = try_ret!(ui::setup_terminal(config.enable_mouse), "Failed to setup terminal");
            let tx_clone = tx.clone();

            let context = try_ret!(
                context::AppContext::try_new(&mut client, config, tx_clone, worker_tx.clone()),
                "Failed to create app context"
            );

            let mut render_loop = RenderLoop::new(worker_tx, context.config);
            if context.status.state == mpd::commands::status::State::Play {
                render_loop.start()?;
            }

            let tx_clone = tx.clone();
            std::thread::Builder::new()
                .name("worker task".to_owned())
                .spawn(|| worker_task(worker_rx, tx_clone, client, context.config))?;

            let tx_clone = tx.clone();

            std::thread::Builder::new()
                .name("input".to_owned())
                .spawn(|| input_poll_task(tx_clone))?;

            let main_task = std::thread::Builder::new().name("main".to_owned()).spawn(|| {
                main_task(context, rx, render_loop, terminal);
            })?;

            let original_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |panic| {
                crossterm::terminal::disable_raw_mode().expect("Disabling of raw mode to succeed");
                crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen)
                    .expect("Exit from alternate screen to succeed");
                original_hook(panic);
            }));

            info!("Application initialized successfully");

            main_task.join().expect("Main task to not fail");
        }
    }

    Ok(())
}

/// first element in return tuple determines whether the result is to be sent into sync work channel
fn handle_work_request(client: &mut Client<'_>, request: WorkRequest, config: &Config) -> Result<WorkDone> {
    match request {
        WorkRequest::DownloadYoutube { url } => {
            YtDlp::download_and_add(config, &url, client)?;

            Ok(WorkDone::None)
        }
        WorkRequest::IndexLyrics { lyrics_dir } => {
            let start = std::time::Instant::now();
            let index = LrcIndex::index(&PathBuf::from(lyrics_dir))?;
            log::info!(found_count = index.len(), elapsed:? = start.elapsed(); "Indexed lrc files");
            Ok(WorkDone::LyricsIndexed { index })
        }
        WorkRequest::MpdQuery(query) => Ok(WorkDone::MpdCommandFinished {
            id: query.id,
            target: query.target,
            data: (query.callback)(client)?,
        }),
        WorkRequest::MpdCommand(command) => {
            (command.callback)(client)?;
            Ok(WorkDone::None)
        }
        WorkRequest::Command(command) => {
            command.execute(client, config)?;
            Ok(WorkDone::None)
        }
    }
}

fn worker_task(rx: Receiver<WorkRequest>, app_event_tx: Sender<AppEvent>, client: Client<'_>, config: &Config) {
    std::thread::scope(move |s| {
        let mut client_write = client.stream.try_clone().expect("Client write clone to succeed");
        let client1 = Arc::new(Mutex::new(client));
        let client2 = client1.clone();
        let app_event_tx2 = app_event_tx.clone();

        let (idle_tx, idle_rx) = bounded::<()>(0);
        let (idle_confirm_tx, idle_confirm_rx) = bounded::<()>(0);

        // TODO any error here should probably end the program right as there is no real way to recover
        // maybe we should display a modal with info which exits the program on confirm
        let idle = Builder::new()
            .name("idle".to_string())
            .spawn_scoped(s, move || loop {
                match idle_rx.recv() {
                    Ok(()) => {
                        let mut client = try_cont!(client1.lock(), "Failed to acquire client lock");
                        let idle_client = try_cont!(client.enter_idle(), "Failed to enter idle state");
                        try_cont!(idle_confirm_tx.send(()), "Failed to send idle confirmation");
                        let evs: Vec<IdleEvent> = try_cont!(idle_client.read_response(), "Failed to read idle events");

                        log::trace!(evs:?; "Got idle events");
                        for ev in evs {
                            try_cont!(app_event_tx2.send(AppEvent::IdleEvent(ev)), "Failed to send idle event");
                        }
                    }
                    Err(err) => {
                        log::error!(err:?; "idle error");
                        break;
                    }
                };
                log::debug!("stopping idle");
            })
            .expect("failed to spawn thread");

        let work = Builder::new()
            .name("work".to_string())
            .spawn_scoped(s, move || {
                let mut buffer = VecDeque::new();

                loop {
                    if let Ok(request) = rx.recv() {
                        buffer.push_back(request);
                    };
                    while let Ok(request) = rx.try_recv() {
                        buffer.push_back(request);
                    }

                    let mut queue: VecDeque<_> = std::mem::take(&mut buffer);

                    try_cont!(client_write.write_all(b"noidle\n"), "Failed to write noidle");
                    let mut client = try_cont!(client2.lock(), "Failed to acquire client lock");

                    while let Some(request) = queue.pop_front() {
                        if let WorkRequest::MpdQuery(MpdQuery { id, target, .. }) = request {
                            if queue.iter().any(|r| match r {
                                WorkRequest::MpdQuery(MpdQuery {
                                    id: id2,
                                    target: target2,
                                    ..
                                }) => id == *id2 && target == *target2,
                                _ => false,
                            }) {
                                // continue;
                            }
                        }

                        match handle_work_request(&mut client, request, config) {
                            Ok(result) => {
                                try_cont!(
                                    app_event_tx.send(AppEvent::WorkDone(Ok(result))),
                                    "Failed to send work done success event"
                                );
                            }
                            Err(err) => {
                                try_cont!(
                                    app_event_tx.send(AppEvent::WorkDone(Err(err))),
                                    "Failed to send work done error event"
                                );
                            }
                        }
                    }
                    drop(client);

                    try_cont!(idle_tx.send(()), "Failed to request for client idle");
                    try_cont!(idle_confirm_rx.recv(), "Idle confirmation failed");
                }
            })
            .expect("failed to spawn thread");

        idle.join().expect("idle thread not to panic");
        work.join().expect("work thread not to panic");
    });
}

fn main_task<B: Backend + std::io::Write>(
    mut context: context::AppContext,
    event_receiver: Receiver<AppEvent>,
    mut render_loop: RenderLoop,
    mut terminal: Terminal<B>,
) {
    let mut ui = Ui::new(&context).expect("UI to be created correctly");
    let event_receiver = event_receiver;
    let mut render_wanted = false;
    let mut full_rerender_wanted = false;
    let max_fps = 30f64;
    let min_frame_duration = Duration::from_secs_f64(1f64 / max_fps);
    let mut last_render = std::time::Instant::now().sub(Duration::from_secs(10));
    let mut additional_evs = HashSet::new();
    ui.before_show(&mut context).expect("Initial render init to succeed");

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
            match event {
                AppEvent::UserKeyInput(key) => match ui.handle_key(&mut key.into(), &mut context) {
                    Ok(ui::KeyHandleResult::None) => continue,
                    Ok(ui::KeyHandleResult::Quit) => {
                        if let Err(err) = ui.on_event(UiEvent::Exit, &mut context) {
                            error!(error:? = err, event:?; "UI failed to handle quit event");
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
                    ui.display_message(message, level);
                    render_wanted = true;
                }
                AppEvent::Log(msg) => {
                    if let Err(err) = ui.on_event(UiEvent::LogAdded(msg), &mut context) {
                        error!(error:? = err; "UI failed to handle log event");
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
                AppEvent::RequestRender(wanted) => {
                    render_wanted = true;
                    full_rerender_wanted = wanted;
                }
                AppEvent::WorkDone(Ok(result)) => match result {
                    WorkDone::LyricsIndexed { index } => {
                        context.lrc_index = index;
                        if let Err(err) = ui.on_event(UiEvent::LyricsIndexed, &mut context) {
                            error!(error:? = err; "UI failed to resize event");
                        }
                    }
                    WorkDone::MpdCommandFinished { id, target, data } => match (id, target, data) {
                        ("global_status_update", None, MpdQueryResult::Status(status)) => {
                            let current_song_id = context.find_current_song_in_queue().map(|(_, song)| song.id);
                            context.status = status;
                            let mut song_changed = false;

                            match context.status.state {
                                State::Play => {
                                    try_skip!(render_loop.start(), "Failed to start render loop");
                                }
                                State::Pause => {
                                    try_skip!(render_loop.stop(), "Failed to stop render loop");
                                }
                                State::Stop => {
                                    song_changed = true;
                                    try_skip!(render_loop.stop(), "Failed to stop render loop");
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
                        ("external_command", None, MpdQueryResult::ExternalCommand(command, songs)) => {
                            let songs = songs.iter().map(|s| s.file.as_str());
                            run_external(command, create_env(&context, songs));
                        }
                        (id, target, data) => {
                            if let Err(err) = ui.on_command_finished(id, target, data, &mut context) {
                                error!(error:? = err; "UI failed to handle command finished event");
                            }
                        }
                    },
                    WorkDone::None => {}
                },
                AppEvent::WorkDone(Err(err)) => {
                    status_error!("{}", err);
                }
                AppEvent::Resized { columns, rows } => {
                    if let Err(err) = ui.on_event(UiEvent::Resized { columns, rows }, &mut context) {
                        error!(error:? = err, event:?; "UI failed to handle resize event");
                    }
                    full_rerender_wanted = true;
                    render_wanted = true;
                }
                AppEvent::UiAppEvent(event) => match ui.on_ui_app_event(event, &mut context) {
                    Ok(()) => {}
                    Err(err) => {
                        status_error!(err:?; "Error: {}", err.to_status());
                        render_wanted = true;
                    }
                },
            }
        }
        if render_wanted {
            let till_next_frame = min_frame_duration.saturating_sub(now.duration_since(last_render));
            if till_next_frame != Duration::ZERO {
                continue;
            }
            if full_rerender_wanted {
                terminal.swap_buffers();
                terminal.swap_buffers();
                terminal.clear().expect("Terminal clear after full rerender to succeed");
                full_rerender_wanted = false;
            }
            terminal
                .draw(|frame| {
                    if let Err(err) = ui.render(frame, &mut context) {
                        error!(error:? = err; "Failed to render a frame");
                    };
                })
                .expect("Expected render to succeed");
            if let Err(err) = ui.post_render(&mut terminal.get_frame(), &mut context) {
                error!(error:? = err; "Failed handle post render phase");
            };

            context.finish_frame();
            last_render = now;
            render_wanted = false;
        }
    }

    ui::restore_terminal(&mut terminal, context.config.enable_mouse).expect("Terminal restore to succeed");
}

fn handle_idle_event(event: IdleEvent, context: &AppContext, result_ui_evs: &mut HashSet<UiEvent>) {
    match event {
        IdleEvent::Mixer => {
            if context.supported_commands.contains("getvol") {
                context.query("global_volume_update", PaneType::Queue, move |client| {
                    Ok(MpdQueryResult::Volume(client.get_volume()?))
                });
                if let Err(err) = context.work_sender.send(WorkRequest::MpdQuery(MpdQuery {
                    id: "global_volume_update",
                    target: None,
                    callback: Box::new(move |client| Ok(MpdQueryResult::Volume(client.get_volume()?))),
                })) {
                    error!(error:? = err; "Failed to send status update request");
                }
            } else if let Err(err) = context.work_sender.send(WorkRequest::MpdQuery(MpdQuery {
                id: "global_status_update",
                target: None,
                callback: Box::new(move |client| Ok(MpdQueryResult::Status(client.get_status()?))),
            })) {
                error!(error:? = err; "Failed to send status update request");
            }
        }
        IdleEvent::Options => {
            if let Err(err) = context.work_sender.send(WorkRequest::MpdQuery(MpdQuery {
                id: "global_status_update",
                target: None,
                callback: Box::new(move |client| Ok(MpdQueryResult::Status(client.get_status()?))),
            })) {
                error!(error:? = err; "Failed to send status update request");
            }
        }
        IdleEvent::Player => {
            if let Err(err) = context.work_sender.send(WorkRequest::MpdQuery(MpdQuery {
                id: "global_status_update",
                target: None,
                callback: Box::new(move |client| Ok(MpdQueryResult::Status(client.get_status()?))),
            })) {
                error!(error:? = err; "Failed to send status update request");
            }
        }
        IdleEvent::Playlist => {
            if let Err(err) = context.work_sender.send(WorkRequest::MpdQuery(MpdQuery {
                id: "global_queue_update",
                target: None,
                callback: Box::new(move |client| Ok(MpdQueryResult::Queue(client.playlist_info()?))),
            })) {
                error!(error:? = err; "Failed to send status update request");
            }
        }
        IdleEvent::StoredPlaylist => {}
        IdleEvent::Database => {}
        IdleEvent::Update => {}
        IdleEvent::Output
        | IdleEvent::Partition
        | IdleEvent::Sticker
        | IdleEvent::Subscription
        | IdleEvent::Message
        | IdleEvent::Neighbor
        | IdleEvent::Mount => {
            warn!(event:?; "Received unhandled event");
        }
    };

    if let Ok(ev) = event.try_into() {
        result_ui_evs.insert(ev);
    }
}

fn input_poll_task(user_input_tx: Sender<AppEvent>) {
    let user_input_tx = user_input_tx;
    let mut mouse_event_tracker = MouseEventTracker::default();
    loop {
        match crossterm::event::poll(Duration::from_millis(250)) {
            Ok(true) => match crossterm::event::read() {
                Ok(Event::Mouse(mouse)) => {
                    if let Some(ev) = mouse_event_tracker.track_and_get(mouse) {
                        if let Err(err) = user_input_tx.send(AppEvent::UserMouseInput(ev)) {
                            error!(error:? = err; "Failed to send user mouse input");
                        }
                    }
                }
                Ok(Event::Key(key)) => {
                    if let Err(err) = user_input_tx.send(AppEvent::UserKeyInput(key)) {
                        error!(error:? = err; "Failed to send user input");
                    }
                }
                Ok(Event::Resize(columns, rows)) => {
                    if let Err(err) = user_input_tx.send(AppEvent::Resized { columns, rows }) {
                        error!(error:? = err; "Failed to render request after resize");
                    }
                }
                Ok(ev) => {
                    log::warn!(ev:?; "Unexpected event");
                }
                Err(err) => {
                    warn!(error:? = err; "Failed to read input event");
                    continue;
                }
            },
            Ok(_) => {}
            Err(e) => warn!(error:? = e; "Error when polling for event"),
        }
    }
}

enum LoopEvent {
    Start,
    Stop,
}

#[derive(Debug)]
struct RenderLoop {
    event_tx: Option<Sender<LoopEvent>>,
}

impl RenderLoop {
    fn new(work_tx: Sender<WorkRequest>, config: &Config) -> Self {
        let (tx, rx) = unbounded::<LoopEvent>();

        // send stop event at the start to not start the loop immedietally
        if let Err(err) = tx.send(LoopEvent::Stop) {
            error!(error:? = err; "Failed to properly initialize status update loop");
        }

        let Some(update_interval) = config.status_update_interval_ms.map(Duration::from_millis) else {
            return Self { event_tx: None };
        };
        std::thread::spawn(move || {
            loop {
                match rx.try_recv() {
                    Ok(LoopEvent::Stop) => loop {
                        if let Ok(LoopEvent::Start) = rx.recv() {
                            break;
                        }
                    },
                    Err(TryRecvError::Disconnected) => {
                        error!("Render loop channel is disconnected");
                    }
                    Ok(LoopEvent::Start) | Err(TryRecvError::Empty) => {} // continue with the update loop
                }

                std::thread::sleep(update_interval);
                if let Err(err) = work_tx.send(WorkRequest::MpdQuery(MpdQuery {
                    id: "global_status_update",
                    target: None,
                    callback: Box::new(move |client| Ok(MpdQueryResult::Status(client.get_status()?))),
                })) {
                    error!(error:? = err; "Failed to send status update request");
                }
            }
        });
        Self { event_tx: Some(tx) }
    }

    fn start(&mut self) -> Result<()> {
        if let Some(tx) = &self.event_tx {
            Ok(tx.send(LoopEvent::Start)?)
        } else {
            Ok(())
        }
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(tx) = &self.event_tx {
            Ok(tx.send(LoopEvent::Stop)?)
        } else {
            Ok(())
        }
    }
}
