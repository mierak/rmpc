use std::{io::Stdout, ops::DerefMut, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use clap::Parser;
use config::Config;
use crossterm::event::{Event, KeyCode, KeyEvent};
use mpd::{client::Client, commands::idle::IdleEvent};
use ratatui::{prelude::CrosstermBackend, Terminal};
use tokio::sync::{mpsc::Sender, Mutex};
use tracing::{debug, error, info, instrument, warn};

use crate::ui::Ui;

mod config;
mod logging;
mod mpd;
mod state;
mod ui;

#[derive(Debug)]
pub enum AppEvent {
    UserInput(Event),
    ErrorInfo(Vec<u8>),
    // TODO there is an issue here
    // if an error is emmited from an ui thread, tracing will notify the thread that it should
    // rerender to show the error which potentionally triggers the error again entering an
    // infinite loop
    // Maybe it could be solved if we can rerender only the status bar since it already is
    // in the shared ui part
    Log(Vec<u8>),
    ClearStatusBar,
    Elapsed(u64),
}

#[derive(Debug)]
struct RenderLoop {
    state: Arc<Mutex<state::State>>,
    render_sender: tokio::sync::mpsc::Sender<()>,
    is_running: Arc<Mutex<bool>>,
}

impl RenderLoop {
    fn new(state: Arc<Mutex<state::State>>, render_sender: tokio::sync::mpsc::Sender<()>) -> Self {
        Self {
            state,
            render_sender,
            is_running: Arc::new(Mutex::new(false)),
        }
    }

    #[instrument(skip(self))]
    async fn render_now(&mut self) {
        if (self.render_sender.send(()).await).is_err() {
            error!("Unable to send render command from status update loop");
        }
    }

    #[instrument(skip(self))]
    async fn start(&mut self) -> bool {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut is_running = self.is_running.lock().await;
        if *is_running {
            return false;
        }
        *is_running.deref_mut() = true;
        let state = Arc::clone(&self.state);
        let running = Arc::clone(&self.is_running);
        let sender = self.render_sender.clone();

        tokio::spawn(async move {
            tracing::debug!("Started status update loop");
            loop {
                interval.tick().await;
                if !*running.lock().await {
                    break;
                }
                let mut state = state.lock().await;
                state.status.elapsed = state.status.elapsed.saturating_add(Duration::from_secs(1));
                if (sender.send(()).await).is_err() {
                    error!("Unable to send render command from status update loop");
                }
            }
            tracing::debug!("Status update loop finished");
        });

        true
    }

    #[instrument(skip(self))]
    async fn stop(&mut self) {
        *self.is_running.lock().await = false;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse();
    let (tx, rx) = tokio::sync::mpsc::channel::<AppEvent>(1024);
    let tx_clone = tx.clone();
    let (render_tx, render_rx) = tokio::sync::mpsc::channel::<()>(8);
    let _guards = logging::configure(config.log, tx.clone());

    let mut client = Client::init(config.mpd_address.clone(), Some("command"), true).await?;
    let terminal = Arc::new(Mutex::new(ui::setup_terminal().unwrap()));
    let state = Arc::new(Mutex::new(state::State::try_new(&mut client).await?));
    let ui = Arc::new(Mutex::new(Ui::new(client)));

    let mut render_loop = RenderLoop::new(Arc::clone(&state), render_tx.clone());
    if state.lock().await.status.state == mpd::commands::status::State::Play {
        render_loop.start().await;
    }

    let main_task = tokio::spawn(main_task(Arc::clone(&ui), Arc::clone(&state), rx, render_tx.clone()));
    let render_task = tokio::task::spawn(render_task(render_rx, ui, Arc::clone(&state), Arc::clone(&terminal)));
    let input_task = tokio::task::spawn_blocking(move || event_poll(tx_clone));
    let idle_task = tokio::task::spawn(idle_task(
        Client::init(config.mpd_address.clone(), Some("idle"), true).await?,
        Client::init(config.mpd_address.clone(), Some("state"), true).await?,
        render_loop,
        Arc::clone(&state),
        render_tx.clone(),
    ));

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        crossterm::terminal::disable_raw_mode().unwrap();
        crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen).unwrap();
        original_hook(panic);
    }));

    info!("Application initialized successfully");
    render_tx.send(()).await?;
    tokio::select! {
        v = idle_task => v,
        v = main_task => v,
        v = render_task => v,
        v = input_task => v,
    }
    .unwrap_or_else(|_| {});

    ui::restore_terminal(&mut *terminal.lock().await).unwrap();

    Ok(())
}

#[instrument(skip_all)]
async fn main_task(
    ui: Arc<Mutex<Ui<'_>>>,
    state2: Arc<Mutex<state::State>>,
    mut event_receiver: tokio::sync::mpsc::Receiver<AppEvent>,
    render_sender: Sender<()>,
) {
    let mut locked_state = state2.lock().await;
    if let Some(selected_id) = locked_state.status.songid {
        if let Some(queue) = locked_state.queue.as_mut() {
            if let Some(song) = queue.0.iter_mut().find(|s| s.id == selected_id) {
                song.selected = true;
            }
        }
    }
    drop(locked_state);

    loop {
        while let Some(event) = event_receiver.recv().await {
            let mut ui = ui.lock().await;
            let mut state = state2.lock().await;

            match event {
                AppEvent::UserInput(Event::Key(key)) => match ui.handle_key(key, &mut state).await {
                    Ok(ui::Render::Skip) => continue,
                    Ok(ui::Render::NoSkip) => {
                        render_sender.send(()).await.unwrap();
                    }
                    Err(err) => {
                        error!(?err);
                        render_sender.send(()).await.unwrap();
                    }
                },
                AppEvent::UserInput(_) => {}
                AppEvent::ErrorInfo(error) => {
                    state.error = error;
                    let state = Arc::clone(&state2);
                    tokio::task::spawn(async move {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        state.lock().await.error.clear();
                    });
                }
                AppEvent::Log(msg) => {
                    state.logs.0.push_back(msg);
                    if state.logs.0.len() > 1000 {
                        state.logs.0.pop_front();
                    }
                }
                AppEvent::ClearStatusBar => {
                    state.error.clear();
                }
                AppEvent::Elapsed(secs) => {
                    state.status.elapsed = state.status.elapsed.saturating_add(Duration::from_secs(secs));
                }
            }
        }
    }
}

#[instrument(skip_all)]
async fn idle_task(
    mut idle_client: Client<'_>,
    mut client: Client<'_>,
    mut render_loop: RenderLoop,
    state1: Arc<Mutex<state::State>>,
    render_sender: Sender<()>,
) {
    loop {
        let events = idle_client
            .idle()
            .await
            .context("Error when unwrapping idle")
            .unwrap()
            .0;

        for event in events {
            let mut state = state1.lock().await;
            debug!(message = "Received idle event", idle_event = ?event);
            match event {
                IdleEvent::Mixer => state.status.volume = client.get_volume().await.unwrap(),
                IdleEvent::Player => {
                    state.current_song = client.get_current_song().await.unwrap();
                    state.status = client.get_status().await.unwrap();
                    if let Some(current_song) = state
                        .queue
                        .as_ref()
                        .and_then(|p| p.0.iter().find(|s| state.status.songid.is_some_and(|i| i == s.id)))
                    {
                        state.album_art = client
                            .find_album_art(&current_song.file)
                            .await
                            .unwrap()
                            .map(state::MyVec);
                    }
                    if state.status.state == mpd::commands::status::State::Play {
                        render_loop.start().await;
                    } else {
                        render_loop.stop().await;
                    }
                }
                IdleEvent::Options => state.status = client.get_status().await.unwrap(),
                IdleEvent::Playlist => state.queue = client.playlist_info().await.unwrap(),
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
                    warn!(message = "Received unhandled event", ?event)
                }
            }
        }
        render_sender.send(()).await.unwrap();
    }
}

#[instrument(skip_all)]
async fn progrss_loop_task(state: Arc<Mutex<state::State>>, render_sender: tokio::sync::mpsc::Sender<()>) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    tracing::debug!("Started status update loop");
    loop {
        interval.tick().await;
        let mut state = state.lock().await;
        let is_playing = state.status.state == mpd::commands::status::State::Play;
        if !is_playing {
            break;
        }
        state.status.elapsed = state.status.elapsed.saturating_add(Duration::from_secs(1));
        if (render_sender.send(()).await).is_err() {
            error!("Unable to send render command from status update loop");
        }
    }
    tracing::debug!("Status update loop finished");
}

#[instrument(skip_all)]
async fn render_task(
    mut render_rx: tokio::sync::mpsc::Receiver<()>,
    ui: Arc<Mutex<Ui<'_>>>,
    state: Arc<Mutex<state::State>>,
    terminal: Arc<Mutex<Terminal<CrosstermBackend<Stdout>>>>,
) {
    while let Some(()) = render_rx.recv().await {
        let mut ui = ui.lock().await;
        let state = state.lock().await;
        let mut terminal = terminal.lock().await;
        ui.render(&mut terminal, &state).expect("Expected render to succeed");
    }
}

#[instrument(skip_all)]
fn event_poll(user_input_tx: Sender<AppEvent>) {
    loop {
        match crossterm::event::poll(Duration::from_millis(250)) {
            Ok(true) => {
                let event = crossterm::event::read().unwrap();
                if let Event::Key(KeyEvent {
                    code: KeyCode::Char('q'),
                    ..
                }) = event
                {
                    break;
                }
                user_input_tx.try_send(AppEvent::UserInput(event)).unwrap();
            }
            Ok(_) => {}
            Err(e) => tracing::error!(message = "Error when polling for event", error = ?e),
        }
    }
}
