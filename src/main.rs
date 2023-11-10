#![deny(clippy::unwrap_used, clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::unused_self,
    clippy::unnested_or_patterns,
    clippy::match_same_arms,
    clippy::manual_let_else,
    clippy::needless_return,
    clippy::zero_sized_map_values,
    clippy::too_many_lines,
    clippy::match_single_binding,
    unused_macros
)]
use std::{ops::Sub, time::Duration};

use anyhow::Result;
use clap::Parser;
use config::{Args, Command, ConfigFile};
use crossterm::event::{Event, KeyEvent};
use mpd::{client::Client, commands::idle::IdleEvent};
use ratatui::{prelude::Backend, Terminal};
use tokio::sync::{
    mpsc::{error::TryRecvError, Sender},
    oneshot::Receiver,
};
use tracing::{error, info, instrument, trace, warn};
use ui::Level;

use crate::{config::Config, mpd::mpd_client::MpdClient, ui::Ui, utils::macros::try_ret};

mod config;
mod logging;
mod mpd;
mod state;
mod ui;
mod utils;

#[derive(Debug)]
pub enum AppEvent {
    UserInput(KeyEvent),
    StatusBar(String),
    // TODO there is an issue here
    // if an error is emmited from an ui thread, tracing will notify the thread that it should
    // rerender to show the error which potentionally triggers the error again entering an
    // infinite loop
    // Maybe it could be solved if we can rerender only the status bar since it already is
    // in the shared ui part
    Log(Vec<u8>),
    IdleEvent(IdleEvent),
    RequestStatusUpdate,
    RequestRender,
}

fn read_cfg(args: &Args) -> Result<Config> {
    let file = std::fs::File::open(&args.config)?;
    let read = std::io::BufReader::new(file);
    let res: ConfigFile = ron::de::from_reader(read)?;
    Ok(res.into())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    match &args.command {
        Some(Command::Config) => {
            println!(
                "{}",
                ron::ser::to_string_pretty(
                    &ConfigFile::default(),
                    ron::ser::PrettyConfig::default()
                        .depth_limit(3)
                        .struct_names(false)
                        .compact_arrays(true),
                )?
            );
            return Ok(());
        }
        None => {
            let (tx, rx) = tokio::sync::mpsc::channel::<AppEvent>(1024);
            let _guards = logging::configure(args.log, &tx.clone());
            let config = Box::leak(Box::new(match read_cfg(&args) {
                Ok(val) => val,
                Err(err) => {
                    warn!(message = "Using default config", ?err);
                    ConfigFile::default().into()
                }
            }));

            try_ret!(tx.send(AppEvent::RequestRender).await, "Failed to render first frame");

            let mut client = try_ret!(
                Client::init(config.address, Some("command"), true).await,
                "Failed to connect to mpd"
            );
            let terminal = try_ret!(ui::setup_terminal(), "Failed to setup terminal");
            let state = try_ret!(
                state::State::try_new(&mut client, config).await,
                "Failed to create app state"
            );

            let mut render_loop = RenderLoop::new(tx.clone());
            if state.status.state == mpd::commands::status::State::Play {
                render_loop.start().await?;
            }

            let (endtx, endrx) = tokio::sync::oneshot::channel::<()>();

            let tx_clone = tx.clone();
            let _input_task = tokio::task::spawn_blocking(move || input_poll_task(tx_clone, endrx));
            let main_task = tokio::spawn(main_task(
                Ui::new(client),
                state,
                rx,
                try_ret!(
                    Client::init(config.address, Some("state"), true).await,
                    "Failed to connect to mpd with state client"
                ),
                render_loop,
                terminal,
            ));
            let idle_task = tokio::task::spawn(idle_task(
                try_ret!(
                    Client::init(config.address, Some("idle"), true).await,
                    "Failed to connect to mpd with idle client"
                ),
                tx,
            ));

            let original_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |panic| {
                crossterm::terminal::disable_raw_mode().expect("Disabling of raw mode to succeed");
                crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen)
                    .expect("Exit from alternate screen to succeed");
                original_hook(panic);
            }));

            info!(message = "Application initialized successfully", ?config);
            try_ret!(
                tokio::select! {
                    v = idle_task => v,
                    v = main_task => v,
                },
                "A task panicked. This should not happen, please check logs."
            );
            try_ret!(endtx.send(()), "Failed to notify event task.");
        }
    }

    Ok(())
}

#[instrument(skip_all)]
async fn main_task<B: Backend + std::io::Write>(
    mut ui: Ui<'static>,
    mut state: state::State,
    mut event_receiver: tokio::sync::mpsc::Receiver<AppEvent>,
    mut client: Client<'_>,
    mut render_loop: RenderLoop,
    mut terminal: Terminal<B>,
) {
    let mut render_wanted = false;
    let max_fps = 30f64;
    let min_frame_duration = Duration::from_secs_f64(1f64 / max_fps);
    let mut last_render = tokio::time::Instant::now().sub(Duration::from_secs(10));

    loop {
        let now = tokio::time::Instant::now();
        let event = if render_wanted {
            tokio::select! {
                () = tokio::time::sleep(min_frame_duration.checked_sub(now - last_render).unwrap_or(Duration::ZERO)) => None,
                v = event_receiver.recv() => v,
            }
        } else {
            event_receiver.recv().await
        };

        if let Some(event) = event {
            match event {
                AppEvent::UserInput(key) => match ui.handle_key(key, &mut state).await {
                    Ok(ui::KeyHandleResult::SkipRender) => continue,
                    Ok(ui::KeyHandleResult::Quit) => break,
                    Ok(ui::KeyHandleResult::RenderRequested) => {
                        render_wanted = true;
                    }
                    Err(err) => {
                        error!(message = "Key handler failed", ?err);
                        render_wanted = true;
                    }
                },
                AppEvent::StatusBar(message) => {
                    ui.display_message(message, Level::Error);
                    render_wanted = true;
                }
                AppEvent::Log(msg) => {
                    state.logs.push_back(msg);
                    if state.logs.len() > 1000 {
                        state.logs.pop_front();
                    }
                }
                AppEvent::IdleEvent(event) => {
                    if let Err(err) = handle_idle_event(event, &mut state, &mut client, &mut render_loop).await {
                        error!(messgae = "Failed handle idle event", error = ?err);
                    }
                    render_wanted = true;
                }
                AppEvent::RequestStatusUpdate => {
                    match client.get_status().await {
                        Ok(status) => state.status = status,
                        Err(err) => {
                            error!(message = "Unable to send render command from status update loop", ?err);
                        }
                    };
                    render_wanted = true;
                }
                AppEvent::RequestRender => {
                    render_wanted = true;
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
                    if let Err(err) = ui.render(frame, &mut state) {
                        error!(message = "Failed to render a frame", error = ?err);
                    };
                })
                .expect("Expected render to succeed");
            last_render = now;
            render_wanted = false;
        }
    }

    ui::restore_terminal(&mut terminal).expect("Terminal restore to succeed");
}

#[instrument]
async fn handle_idle_event(
    event: IdleEvent,
    state: &mut state::State,
    client: &mut Client<'_>,
    render_loop: &mut RenderLoop,
) -> Result<()> {
    match event {
        IdleEvent::Mixer => state.status.volume = try_ret!(client.get_volume().await, "Failed to get volume"),
        IdleEvent::Player => {
            state.current_song = try_ret!(client.get_current_song().await, "Failed get current song");
            state.status = try_ret!(client.get_status().await, "Failed get status");
            if let Some(current_song) = state
                .queue
                .as_ref()
                .and_then(|p| p.iter().find(|s| state.status.songid.is_some_and(|i| i == s.id)))
            {
                if !state.config.disable_images {
                    state.album_art = try_ret!(
                        client.find_album_art(&current_song.file).await,
                        "Failed to get find album art"
                    )
                    .map(state::MyVec::new);
                }
            }
            if state.status.state == mpd::commands::status::State::Play {
                render_loop.start().await?;
            } else {
                render_loop.stop().await?;
            }
        }
        IdleEvent::Options => state.status = try_ret!(client.get_status().await, "Failed to get status"),
        IdleEvent::Playlist => state.queue = try_ret!(client.playlist_info().await, "Failed to get playlist"),
        // TODO: handle these events eventually ?
        IdleEvent::Database => warn!(message = "Received unhandled event", ?event),
        IdleEvent::Update => warn!(message = "Received unhandled event", ?event),
        IdleEvent::Output => warn!(message = "Received unhandled event", ?event),
        IdleEvent::Partition => warn!(message = "Received unhandled event", ?event),
        IdleEvent::Sticker => warn!(message = "Received unhandled event", ?event),
        IdleEvent::Subscription => warn!(message = "Received unhandled event", ?event),
        IdleEvent::Message => warn!(message = "Received unhandled event", ?event),
        IdleEvent::Neighbor => warn!(message = "Received unhandled event", ?event),
        IdleEvent::Mount => warn!(message = "Received unhandled event", ?event),
        IdleEvent::StoredPlaylist => {
            warn!(message = "Received unhandled event", ?event);
        }
    };
    Ok(())
}

#[instrument(skip_all, fields(events))]
async fn idle_task(mut idle_client: Client<'_>, sender: Sender<AppEvent>) {
    let mut error_count = 0;
    loop {
        let events = match idle_client.idle().await {
            Ok(val) => val,
            Err(err) => {
                if error_count > 5 {
                    error!(message = "Unexpected error when receiving idle events", ?err);
                    break;
                }
                warn!(message = "Unexpected error when receiving idle events", ?err);
                error_count += 1;
                tokio::time::sleep(Duration::from_secs(error_count)).await;
                continue;
            }
        };

        for event in events {
            trace!(message = "Received idle event", idle_event = ?event);
            if let Err(err) = sender.send(AppEvent::IdleEvent(event)).await {
                error!(messgae = "Failed to send app event", error = ?err);
            }
        }
    }
}

#[instrument(skip_all)]
fn input_poll_task(user_input_tx: Sender<AppEvent>, mut end: Receiver<()>) {
    loop {
        if let Ok(()) = end.try_recv() {
            break;
        }
        match crossterm::event::poll(Duration::from_millis(250)) {
            Ok(true) => {
                let event = match crossterm::event::read() {
                    Ok(e) => e,
                    Err(err) => {
                        warn!(messgae = "Failed to read input event", error = ?err);
                        continue;
                    }
                };
                if let Event::Key(key) = event {
                    if let Err(err) = user_input_tx.try_send(AppEvent::UserInput(key)) {
                        error!(messgae = "Failed to send user input", error = ?err);
                    }
                }
            }
            Ok(_) => {}
            Err(e) => warn!(message = "Error when polling for event", error = ?e),
        }
    }
}

enum LoopEvent {
    Start,
    Stop,
}

#[derive(Debug)]
struct RenderLoop {
    event_tx: tokio::sync::mpsc::Sender<LoopEvent>,
}

impl RenderLoop {
    fn new(render_sender: tokio::sync::mpsc::Sender<AppEvent>) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<LoopEvent>(32);

        // send stop event at the start to not start the loop immedietally
        if let Err(err) = tx.try_send(LoopEvent::Stop) {
            error!(messgae = "Failed to properly initialize status update loop", error = ?err);
        }

        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                match rx.try_recv() {
                    Ok(LoopEvent::Stop) => loop {
                        if let Some(LoopEvent::Start) = rx.recv().await {
                            break;
                        }
                    },
                    Err(TryRecvError::Disconnected) => {
                        error!("Render loop channel is disconnected");
                    }
                    Ok(LoopEvent::Start) | Err(TryRecvError::Empty) => {} // continue with the update loop
                }

                interval.tick().await;
                if let Err(err) = render_sender.send(AppEvent::RequestStatusUpdate).await {
                    error!(messgae = "Failed to send status update request", error = ?err);
                }
            }
        });
        Self { event_tx: tx }
    }

    async fn start(&mut self) -> Result<()> {
        Ok(self.event_tx.send(LoopEvent::Start).await?)
    }

    #[instrument(skip(self))]
    async fn stop(&mut self) -> Result<()> {
        Ok(self.event_tx.send(LoopEvent::Stop).await?)
    }
}
