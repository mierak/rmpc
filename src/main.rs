#![deny(clippy::unwrap_used, clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::unused_self,
    clippy::unnested_or_patterns,
    clippy::match_same_arms,
    clippy::manual_let_else,
    clippy::needless_return,
    clippy::zero_sized_map_values
)]
use std::{sync::Arc, time::Duration};

use anyhow::Result;
use clap::Parser;
use config::{Args, Command, ConfigFile};
use crossterm::event::{Event, KeyCode, KeyEvent};
use mpd::{client::Client, commands::idle::IdleEvent};
use ratatui::{prelude::Backend, Terminal};
use tokio::{
    sync::{mpsc::Sender, Mutex},
    task::JoinHandle,
};
use tracing::{debug, error, info, instrument, trace, warn};
use ui::Level;

use crate::{
    config::Config,
    mpd::mpd_client::MpdClient,
    ui::Ui,
    utils::macros::{try_cont, try_ret},
};

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
            let tx_clone = tx.clone();
            let (render_tx, render_rx) = tokio::sync::mpsc::channel::<()>(1024);
            let is_aborted = Arc::new(Mutex::new(false));
            let config = Box::leak(Box::new(match read_cfg(&args) {
                Ok(val) => val,
                Err(err) => {
                    warn!(message = "Using default config", ?err);
                    ConfigFile::default().into()
                }
            }));

            let mut client = try_ret!(
                Client::init(config.address, Some("command"), true).await,
                "Failed to connect to mpd"
            );
            let terminal = Arc::new(Mutex::new(try_ret!(ui::setup_terminal(), "Failed to setup terminal")));
            let state = Arc::new(Mutex::new(try_ret!(
                state::State::try_new(&mut client, config).await,
                "Failed to create app state"
            )));
            let ui = Arc::new(Mutex::new(Ui::new(client)));

            let mut render_loop = RenderLoop::new(Arc::clone(&state), render_tx.clone());
            if state.lock().await.status.state == mpd::commands::status::State::Play {
                render_loop.start(config.address).await;
            }

            let main_task = tokio::spawn(main_task(Arc::clone(&ui), Arc::clone(&state), rx, render_tx.clone()));
            let render_task = tokio::task::spawn(render_task(render_rx, ui, Arc::clone(&state), Arc::clone(&terminal)));
            let ab = Arc::clone(&is_aborted);
            let input_task = tokio::task::spawn_blocking(move || event_poll(tx_clone, ab));
            let idle_task = tokio::task::spawn(idle_task(
                try_ret!(
                    Client::init(config.address, Some("idle"), true).await,
                    "Failed to connect to mpd with idle client"
                ),
                try_ret!(
                    Client::init(config.address, Some("state"), true).await,
                    "Failed to connect to mpd with state client"
                ),
                render_loop,
                Arc::clone(&state),
                render_tx.clone(),
            ));

            let original_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |panic| {
                crossterm::terminal::disable_raw_mode().expect("Disabling of raw mode to succeed");
                crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen)
                    .expect("Exit from alternate screen to succeed");
                original_hook(panic);
            }));

            info!(message = "Application initialized successfully", ?config);
            try_ret!(render_tx.send(()).await, "Failed to render first frame");
            try_ret!(
                tokio::select! {
                    v = idle_task => v,
                    v = main_task => v,
                    v = render_task => v,
                    v = input_task => v,
                },
                "A task panicked. This should not happen, please check logs."
            );
            *is_aborted.lock().await = true;

            ui::restore_terminal(&mut *terminal.lock().await).expect("Terminal restore to succeed");
        }
    }

    Ok(())
}

#[instrument(skip_all)]
async fn main_task(
    ui_mutex: Arc<Mutex<Ui<'static>>>,
    state2: Arc<Mutex<state::State>>,
    mut event_receiver: tokio::sync::mpsc::Receiver<AppEvent>,
    render_sender: Sender<()>,
) {
    loop {
        while let Some(event) = event_receiver.recv().await {
            let mut state = state2.lock().await;
            let mut ui = ui_mutex.lock().await;

            match event {
                AppEvent::UserInput(key) => match ui.handle_key(key, &mut state).await {
                    Ok(ui::KeyHandleResult::KeyNotHandled) => continue,
                    Ok(ui::KeyHandleResult::SkipRender) => continue,
                    Ok(ui::KeyHandleResult::RenderRequested) => {
                        if let Err(err) = render_sender.send(()).await {
                            error!(messgae = "Failed to send render request", error = ?err);
                        }
                    }
                    Err(err) => {
                        error!(message = "Key handler failed", ?err);
                        if let Err(err) = render_sender.send(()).await {
                            error!(messgae = "Failed to send render request", error = ?err);
                        }
                    }
                },
                AppEvent::StatusBar(message) => {
                    ui.display_message(message, Level::Error);
                }
                AppEvent::Log(msg) => {
                    state.logs.0.push_back(msg);
                    if state.logs.0.len() > 1000 {
                        state.logs.0.pop_front();
                    }
                }
            }
        }
    }
}

#[instrument(skip_all, fields(events))]
async fn idle_task(
    mut idle_client: Client<'_>,
    mut client: Client<'_>,
    mut render_loop: RenderLoop,
    state1: Arc<Mutex<state::State>>,
    render_sender: Sender<()>,
) {
    let mut error_count = 0;
    loop {
        let events = match idle_client.idle().await {
            Ok(val) => val.0,
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
            let mut state = state1.lock().await;
            trace!(message = "Received idle event", idle_event = ?event);
            match event {
                IdleEvent::Mixer => state.status.volume = try_cont!(client.get_volume().await, "Failed to get volume"),
                IdleEvent::Player => {
                    state.current_song = try_cont!(client.get_current_song().await, "Failed get current song");
                    state.status = try_cont!(client.get_status().await, "Failed get status");
                    if let Some(current_song) = state
                        .queue
                        .as_ref()
                        .and_then(|p| p.0.iter().find(|s| state.status.songid.is_some_and(|i| i == s.id)))
                    {
                        if !state.config.disable_images {
                            state.album_art = try_cont!(
                                client.find_album_art(&current_song.file).await,
                                "Failed to get find album art"
                            )
                            .map(state::MyVec);
                        }
                    }
                    if state.status.state == mpd::commands::status::State::Play {
                        render_loop.start(state.config.address).await;
                    } else {
                        render_loop.stop().await;
                    }
                }
                IdleEvent::Options => state.status = try_cont!(client.get_status().await, "Failed to get status"),
                IdleEvent::Playlist => state.queue = try_cont!(client.playlist_info().await, "Failed to get playlist"),
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
        }
        if let Err(err) = render_sender.send(()).await {
            tracing::warn!(message = "Failed to send render request", ?err);
        };
    }
}

#[instrument(skip_all)]
async fn render_task<B: Backend>(
    mut render_rx: tokio::sync::mpsc::Receiver<()>,
    ui: Arc<Mutex<Ui<'_>>>,
    state: Arc<Mutex<state::State>>,
    terminal: Arc<Mutex<Terminal<B>>>,
) {
    {
        let mut state = state.lock().await;
        let mut ui = ui.lock().await;
        if let Err(err) = ui.before_show(&mut state).await {
            error!(message = "Failed to render a frame!!!", error = ?err);
        };
    }

    while let Some(()) = render_rx.recv().await {
        let mut state = state.lock().await;
        let mut ui = ui.lock().await;
        let mut terminal = terminal.lock().await;
        terminal
            .draw(|frame| {
                if let Err(err) = ui.render(frame, &mut state) {
                    error!(message = "Failed to render a frame!!!", error = ?err);
                };
            })
            .expect("Expected render to succeed");
    }
}

#[instrument(skip_all)]
fn event_poll(user_input_tx: Sender<AppEvent>, is_aborted: Arc<Mutex<bool>>) {
    loop {
        if *is_aborted.blocking_lock() {
            debug!(message = "Event poll loop ended because it was aborted");
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
                match event {
                    Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        ..
                    }) => break,
                    Event::Key(key) => {
                        if let Err(err) = user_input_tx.try_send(AppEvent::UserInput(key)) {
                            error!(messgae = "Failed to send user input", error = ?err);
                        }
                    }
                    _ => {} // ignore other events
                }
            }
            Ok(_) => {}
            Err(e) => warn!(message = "Error when polling for event", error = ?e),
        }
    }
}

#[derive(Debug)]
struct RenderLoop {
    state: Arc<Mutex<state::State>>,
    render_sender: tokio::sync::mpsc::Sender<()>,
    handle: Option<JoinHandle<()>>,
}

impl RenderLoop {
    fn new(state: Arc<Mutex<state::State>>, render_sender: tokio::sync::mpsc::Sender<()>) -> Self {
        Self {
            state,
            render_sender,
            handle: None,
        }
    }

    #[instrument(skip(self))]
    async fn render_now(&mut self) {
        if (self.render_sender.send(()).await).is_err() {
            error!("Unable to send render command from status update loop");
        }
    }

    async fn start(&mut self, addr: &'static str) -> bool {
        if self.handle.is_none() {
            debug!("Started update loop");

            let state = Arc::clone(&self.state);
            let sender = self.render_sender.clone();

            let mut interval = tokio::time::interval(Duration::from_secs(1));
            self.handle = Some(tokio::spawn(async move {
                let mut client = match Client::init(addr, Some("status_loop"), true).await {
                    Ok(client) => client,
                    Err(e) => {
                        error!(message = "Unable to start status update loop", ?e);
                        return;
                    }
                };
                debug!("Started status update loop");
                let mut error_count: u8 = 0;
                loop {
                    interval.tick().await;
                    let mut state = state.lock().await;
                    let status = client.get_status().await;
                    match status {
                        Ok(status) => {
                            state.status = status;
                            if (sender.send(()).await).is_err() {
                                error_count += 1;
                                error!(
                                    message = "Unable to send render command from status update loop",
                                    error_count
                                );
                            }
                        }
                        Err(err) => {
                            error_count += 1;
                            error!(
                                message = "Unable to send render command from status update loop",
                                ?err,
                                error_count
                            );
                        }
                    }
                    if error_count > 5 {
                        error!(
                            message = "Status update loop cancelled after retries were exhausted",
                            error_count
                        );
                        break;
                    }
                }
            }));
            true
        } else {
            false
        }
    }

    #[instrument(skip(self))]
    async fn stop(&mut self) {
        if let Some(handle) = &self.handle {
            handle.abort();
            self.handle = None;
        }
    }
}
