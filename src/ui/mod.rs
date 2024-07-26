use std::{io::Stdout, ops::AddAssign, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use modals::{keybinds::KeybindsModal, outputs::OutputsModal};
use ratatui::{
    prelude::{Backend, Constraint, CrosstermBackend, Layout},
    style::{Color, Style},
    symbols::border,
    text::Text,
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use widgets::app_tabs::AppTabs;

use crate::{
    config::{
        keys::{CommonAction, GlobalAction},
        Args, Config,
    },
    mpd::{
        client::Client,
        commands::{idle::IdleEvent, volume::Bound, State as MpdState},
        mpd_client::{FilterKind, MpdClient, ValueChange},
    },
    utils::macros::{status_error, try_ret},
    AppEvent, WorkRequest,
};
use crate::{mpd::version::Version, state::State};

#[cfg(debug_assertions)]
use self::screens::logs::LogsScreen;
use self::{
    modals::Modal,
    screens::{
        albums::AlbumsScreen, artists::ArtistsScreen, directories::DirectoriesScreen, playlists::PlaylistsScreen,
        queue::QueueScreen, search::SearchScreen, Screen,
    },
    widgets::header::Header,
};

pub mod modals;
pub mod screens;
pub mod utils;
pub mod widgets;

#[derive(Debug)]
#[allow(dead_code)]
pub enum Level {
    Trace,
    Debug,
    Warn,
    Error,
    Info,
}

#[derive(Debug)]
pub struct StatusMessage {
    pub message: String,
    pub level: Level,
    pub created: std::time::Instant,
}

#[derive(Debug)]
pub struct Ui {
    screens: Screens,
    modals: Vec<Box<dyn Modal>>,
    active_screen: screens::Screens,
    status_message: Option<StatusMessage>,
    rendered_frames_count: u32,
    current_song: Option<crate::mpd::commands::Song>,
    command: Option<String>,
    work_request_sender: std::sync::mpsc::Sender<WorkRequest>,
}

impl Ui {
    pub fn new(
        config: &Config,
        app_event_sender: std::sync::mpsc::Sender<AppEvent>,
        work_request_sender: std::sync::mpsc::Sender<WorkRequest>,
    ) -> Ui {
        Self {
            work_request_sender,
            screens: Screens::new(config, app_event_sender),
            active_screen: screens::Screens::Queue,
            status_message: None,
            rendered_frames_count: 0,
            current_song: None,
            modals: Vec::default(),
            command: None,
        }
    }
}

#[derive(Debug)]
struct Screens {
    queue: QueueScreen,
    #[cfg(debug_assertions)]
    logs: LogsScreen,
    directories: DirectoriesScreen,
    albums: AlbumsScreen,
    artists: ArtistsScreen,
    playlists: PlaylistsScreen,
    search: SearchScreen,
}

impl Screens {
    fn new(config: &Config, app_event_sender: std::sync::mpsc::Sender<AppEvent>) -> Self {
        Self {
            queue: QueueScreen::new(config, app_event_sender),
            #[cfg(debug_assertions)]
            logs: LogsScreen::default(),
            directories: DirectoriesScreen::default(),
            albums: AlbumsScreen::default(),
            artists: ArtistsScreen::default(),
            playlists: PlaylistsScreen::default(),
            search: SearchScreen::default(),
        }
    }
}

macro_rules! invoke {
    ($screen:expr, $fn:ident, $($param:expr),+) => {
        $screen.$fn($($param),+)
    };
}

macro_rules! screen_call {
    ($self:ident, $state:ident, $fn:ident($($param:expr),+)) => {
        match $self.active_screen {
            screens::Screens::Queue => invoke!($self.screens.queue, $fn, $($param),+),
            #[cfg(debug_assertions)]
            screens::Screens::Logs => invoke!($self.screens.logs, $fn, $($param),+),
            screens::Screens::Directories => invoke!($self.screens.directories, $fn, $($param),+),
            screens::Screens::Artists => invoke!($self.screens.artists, $fn, $($param),+),
            screens::Screens::Albums => invoke!($self.screens.albums, $fn, $($param),+),
            screens::Screens::Playlists => invoke!($self.screens.playlists, $fn, $($param),+),
            screens::Screens::Search => invoke!($self.screens.search, $fn, $($param),+),
        }
    }
}

impl Ui {
    pub fn render(&mut self, frame: &mut Frame, state: &mut State) -> Result<()> {
        if let Some(bg_color) = state.config.theme.background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), frame.size());
        }
        self.rendered_frames_count.add_assign(1);
        if self
            .status_message
            .as_ref()
            .is_some_and(|m| m.created.elapsed() > std::time::Duration::from_secs(5))
        {
            self.status_message = None;
        }

        let tab_area_height = match (state.config.theme.tab_bar.enabled, state.config.theme.draw_borders) {
            (true, true) => 3,
            (true, false) => 1,
            (false, _) => 0,
        };
        let [header_area, tabs_area, content_area, bar_area] = *Layout::vertical([
            Constraint::Length(u16::try_from(state.config.theme.header.rows.len())?),
            Constraint::Length(tab_area_height), // Tab bar
            Constraint::Percentage(100),
            Constraint::Min(1),
        ])
        .split(frame.size()) else {
            return Ok(());
        };

        let header = Header::new(state.config, &state.status, self.current_song.as_ref());
        frame.render_widget(header, header_area);

        if tab_area_height > 0 {
            let app_tabs = AppTabs::new(self.active_screen, state.config);
            frame.render_widget(app_tabs, tabs_area);
        }

        if let Some(command) = &self.command {
            let [leader_area, command_area] =
                *Layout::horizontal([Constraint::Length(1), Constraint::Percentage(100)]).split(bar_area)
            else {
                return Ok(());
            };

            let status_bar = Paragraph::new(command.as_str())
                .alignment(ratatui::prelude::Alignment::Left)
                .style(state.config.as_text_style());

            frame.render_widget(Text::from(":"), leader_area); // TODO: use key from config
            frame.render_widget(status_bar, command_area);
        } else if let Some(StatusMessage { message, level, .. }) = &self.status_message {
            let status_bar = Paragraph::new(message.to_owned())
                .alignment(ratatui::prelude::Alignment::Center)
                .style(Style::default().fg(level.into()).bg(Color::Black));
            frame.render_widget(status_bar, bar_area);
        } else if state.config.status_update_interval_ms.is_some() {
            let elapsed_bar = state.config.as_styled_progress_bar();
            let elapsed_bar = if state.status.duration == Duration::ZERO {
                elapsed_bar.value(0.0)
            } else {
                elapsed_bar.value(state.status.elapsed.as_secs_f32() / state.status.duration.as_secs_f32())
            };
            frame.render_widget(elapsed_bar, bar_area);
        }

        // #[cfg(debug_assertions)]
        // frame.render_widget(
        //     Paragraph::new(format!("{} frames", self.rendered_frames_count)),
        //     bar_area,
        // );

        if state.config.theme.draw_borders {
            screen_call!(self, state, render(frame, content_area, &state.status, state.config))?;
        } else {
            screen_call!(
                self,
                state,
                render(
                    frame,
                    ratatui::prelude::Rect {
                        x: content_area.x,
                        y: content_area.y,
                        width: content_area.width,
                        height: content_area.height,
                    },
                    &state.status,
                    state.config
                )
            )?;
        }

        for modal in &mut self.modals {
            modal.render(frame, state)?;
        }

        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyEvent, state: &mut State, client: &mut Client<'_>) -> Result<KeyHandleResult> {
        let action = state.config.keybinds.navigation.get(&key.into());
        if let Some(ref mut command) = self.command {
            if let Some(CommonAction::Close) = action {
                self.command = None;
                return Ok(KeyHandleResult::RenderRequested);
            } else if let Some(CommonAction::Confirm) = action {
                let cmd = command.parse();
                log::debug!("Executing command: {:?}", cmd);

                self.command = None;
                match cmd {
                    Ok(Args { command: Some(cmd), .. }) => {
                        cmd.execute(client, state.config, |request, _| {
                            if let Err(err) = self.work_request_sender.send(request) {
                                status_error!("Failed to send work request: {}", err);
                            }
                        })?;
                    }
                    Err(err) => {
                        status_error!("Failed to parse command. {:?}", err);
                    }
                    _ => {}
                }

                return Ok(KeyHandleResult::RenderRequested);
            }

            match key.code {
                KeyCode::Char(c) => {
                    command.push(c);
                    return Ok(KeyHandleResult::RenderRequested);
                }
                KeyCode::Backspace => {
                    command.pop();
                    return Ok(KeyHandleResult::RenderRequested);
                }
                _ => return Ok(KeyHandleResult::SkipRender),
            }
        }

        macro_rules! screen_call_inner {
            ($fn:ident($($param:expr),+)) => {
                screen_call!(self, state, $fn($($param),+))?
            }
        }
        if let Some(ref mut modal) = self.modals.last_mut() {
            return match modal.handle_key(key, client, state)? {
                KeyHandleResultInternal::Modal(None) => {
                    self.modals.pop();
                    Ok(KeyHandleResult::RenderRequested)
                }
                r => Ok(r.into()),
            };
        }

        match screen_call_inner!(handle_action(key, client, &mut state.status, state.config)) {
            KeyHandleResultInternal::RenderRequested => return Ok(KeyHandleResult::RenderRequested),
            KeyHandleResultInternal::SkipRender => return Ok(KeyHandleResult::SkipRender),
            KeyHandleResultInternal::Modal(Some(modal)) => {
                self.modals.push(modal);
                return Ok(KeyHandleResult::RenderRequested);
            }
            KeyHandleResultInternal::Modal(None) => {
                self.modals.pop();
                return Ok(KeyHandleResult::RenderRequested);
            }
            KeyHandleResultInternal::KeyNotHandled => {
                if let Some(action) = state.config.keybinds.global.get(&key.into()) {
                    match action {
                        GlobalAction::Command { command, .. } => {
                            let cmd = command.parse();
                            log::debug!("executing {:?}", cmd);

                            self.command = None;
                            if let Ok(Args { command: Some(cmd), .. }) = cmd {
                                cmd.execute(client, state.config, |request, _| {
                                    if let Err(err) = self.work_request_sender.send(request) {
                                        status_error!("Failed to send work request: {}", err);
                                    }
                                })?;
                            }
                        }
                        GlobalAction::CommandMode => {
                            self.command = Some(String::new());
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::NextTrack if state.status.state == MpdState::Play => client.next()?,
                        GlobalAction::PreviousTrack if state.status.state == MpdState::Play => client.prev()?,
                        GlobalAction::Stop if state.status.state == MpdState::Play => client.stop()?,
                        GlobalAction::ToggleRepeat => client.repeat(!state.status.repeat)?,
                        GlobalAction::ToggleSingle => client.single(state.status.single.cycle())?,
                        GlobalAction::ToggleRandom => client.random(!state.status.random)?,
                        GlobalAction::ToggleConsume if client.version() < Version::new(0, 24, 0) => {
                            client.consume(state.status.consume.cycle_pre_mpd_24())?;
                        }
                        GlobalAction::ToggleConsume => {
                            client.consume(state.status.consume.cycle())?;
                        }
                        GlobalAction::TogglePause
                            if state.status.state == MpdState::Play || state.status.state == MpdState::Pause =>
                        {
                            client.pause_toggle()?;
                            return Ok(KeyHandleResult::SkipRender);
                        }
                        GlobalAction::TogglePause => {}
                        GlobalAction::VolumeUp => {
                            client.set_volume(*state.status.volume.inc_by(state.config.volume_step))?;
                        }
                        GlobalAction::VolumeDown => {
                            client.set_volume(*state.status.volume.dec_by(state.config.volume_step))?;
                        }
                        GlobalAction::SeekForward if state.status.state == MpdState::Play => {
                            client.seek_current(ValueChange::Increase(5))?;
                        }
                        GlobalAction::SeekBack if state.status.state == MpdState::Play => {
                            client.seek_current(ValueChange::Decrease(5))?;
                        }
                        GlobalAction::NextTab => {
                            screen_call_inner!(on_hide(client, &mut state.status, state.config));

                            self.active_screen = self.active_screen.next();
                            screen_call_inner!(before_show(client, &mut state.status, state.config));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::PreviousTab => {
                            screen_call_inner!(on_hide(client, &mut state.status, state.config));

                            self.active_screen = self.active_screen.prev();
                            screen_call_inner!(before_show(client, &mut state.status, state.config));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::QueueTab if !matches!(self.active_screen, screens::Screens::Queue) => {
                            screen_call_inner!(on_hide(client, &mut state.status, state.config));

                            self.active_screen = screens::Screens::Queue;
                            screen_call_inner!(before_show(client, &mut state.status, state.config));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::DirectoriesTab
                            if !matches!(self.active_screen, screens::Screens::Directories) =>
                        {
                            screen_call_inner!(on_hide(client, &mut state.status, state.config));

                            self.active_screen = screens::Screens::Directories;
                            screen_call_inner!(before_show(client, &mut state.status, state.config));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::ArtistsTab if !matches!(self.active_screen, screens::Screens::Artists) => {
                            screen_call_inner!(on_hide(client, &mut state.status, state.config));

                            self.active_screen = screens::Screens::Artists;
                            screen_call_inner!(before_show(client, &mut state.status, state.config));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::AlbumsTab if !matches!(self.active_screen, screens::Screens::Albums) => {
                            screen_call_inner!(on_hide(client, &mut state.status, state.config));

                            self.active_screen = screens::Screens::Albums;
                            screen_call_inner!(before_show(client, &mut state.status, state.config));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::PlaylistsTab if !matches!(self.active_screen, screens::Screens::Playlists) => {
                            screen_call_inner!(on_hide(client, &mut state.status, state.config));

                            self.active_screen = screens::Screens::Playlists;
                            screen_call_inner!(before_show(client, &mut state.status, state.config));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::SearchTab if !matches!(self.active_screen, screens::Screens::Search) => {
                            screen_call_inner!(on_hide(client, &mut state.status, state.config));

                            self.active_screen = screens::Screens::Search;
                            screen_call_inner!(before_show(client, &mut state.status, state.config));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::QueueTab => {}
                        GlobalAction::DirectoriesTab => {}
                        GlobalAction::ArtistsTab => {}
                        GlobalAction::AlbumsTab => {}
                        GlobalAction::PlaylistsTab => {}
                        GlobalAction::SearchTab => {}
                        GlobalAction::NextTrack => {}
                        GlobalAction::PreviousTrack => {}
                        GlobalAction::Stop => {}
                        GlobalAction::SeekBack => {}
                        GlobalAction::SeekForward => {}
                        GlobalAction::Quit => return Ok(KeyHandleResult::Quit),
                        GlobalAction::ShowHelp => {
                            self.modals.push(Box::new(KeybindsModal::new(state)));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::ShowOutputs => {
                            self.modals.push(Box::new(OutputsModal::new(client.outputs()?.0)));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                    }
                    Ok(KeyHandleResult::SkipRender)
                } else {
                    Ok(KeyHandleResult::SkipRender)
                }
            }
        }
    }

    pub fn before_show(&mut self, state: &mut State, client: &mut impl MpdClient) -> Result<()> {
        self.current_song = try_ret!(client.get_current_song(), "Failed get current song");
        screen_call!(self, state, before_show(client, &mut state.status, state.config))
    }

    pub fn display_message(&mut self, message: String, level: Level) {
        self.status_message = Some(StatusMessage {
            message,
            level,
            created: std::time::Instant::now(),
        });
    }

    pub fn on_event(
        &mut self,
        mut event: UiEvent,
        state: &mut State,
        client: &mut impl MpdClient,
    ) -> Result<KeyHandleResult> {
        match event {
            UiEvent::Mixer => state.status.volume = try_ret!(client.get_volume(), "Failed to get volume"),
            UiEvent::Options => state.status = try_ret!(client.get_status(), "Failed to get status"),
            UiEvent::Player => {
                self.current_song = try_ret!(client.get_current_song(), "Failed get current song");
            }
            UiEvent::Playlist => {}
            UiEvent::Database => {}
            UiEvent::StoredPlaylist => {}
            UiEvent::LogAdded(_) => {}
            UiEvent::Update => {}
        }

        let mut ret = KeyHandleResultInternal::SkipRender;

        for screen in [
            screens::Screens::Queue,
            #[cfg(debug_assertions)]
            screens::Screens::Logs,
            screens::Screens::Directories,
            screens::Screens::Albums,
            screens::Screens::Artists,
            screens::Screens::Playlists,
            screens::Screens::Search,
        ] {
            let result = match screen {
                #[cfg(debug_assertions)]
                screens::Screens::Logs => {
                    self.screens
                        .logs
                        .on_event(&mut event, client, &mut state.status, state.config)
                }
                screens::Screens::Queue => {
                    self.screens
                        .queue
                        .on_event(&mut event, client, &mut state.status, state.config)
                }
                screens::Screens::Directories => {
                    self.screens
                        .directories
                        .on_event(&mut event, client, &mut state.status, state.config)
                }
                screens::Screens::Albums => {
                    self.screens
                        .albums
                        .on_event(&mut event, client, &mut state.status, state.config)
                }
                screens::Screens::Artists => {
                    self.screens
                        .artists
                        .on_event(&mut event, client, &mut state.status, state.config)
                }
                screens::Screens::Playlists => {
                    self.screens
                        .playlists
                        .on_event(&mut event, client, &mut state.status, state.config)
                }
                screens::Screens::Search => {
                    self.screens
                        .search
                        .on_event(&mut event, client, &mut state.status, state.config)
                }
            };

            match self.handle_screen_event_result(result)? {
                KeyHandleResult::RenderRequested => ret = KeyHandleResultInternal::RenderRequested,
                KeyHandleResult::SkipRender => {}
                KeyHandleResult::Quit => {}
            }
        }

        Ok(ret.into())
    }

    fn handle_screen_event_result(&mut self, result: Result<KeyHandleResultInternal>) -> Result<KeyHandleResult> {
        match result {
            Ok(KeyHandleResultInternal::SkipRender) => Ok(KeyHandleResult::SkipRender),
            Ok(KeyHandleResultInternal::RenderRequested) => Ok(KeyHandleResult::RenderRequested),
            Ok(KeyHandleResultInternal::Modal(Some(modal))) => {
                self.modals.push(modal);
                Ok(KeyHandleResult::RenderRequested)
            }
            Ok(KeyHandleResultInternal::Modal(None)) => {
                self.modals.pop();
                Ok(KeyHandleResult::RenderRequested)
            }
            Ok(KeyHandleResultInternal::KeyNotHandled) => Ok(KeyHandleResult::SkipRender),
            Err(err) => Err(err),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum UiEvent {
    Player,
    Mixer,
    Playlist,
    Options,
    Database,
    StoredPlaylist,
    Update,
    LogAdded(Vec<u8>),
}

impl TryFrom<IdleEvent> for UiEvent {
    type Error = ();

    fn try_from(event: IdleEvent) -> Result<Self, ()> {
        Ok(match event {
            IdleEvent::Player => UiEvent::Player,
            IdleEvent::Update => UiEvent::Update,
            IdleEvent::Mixer => UiEvent::Mixer,
            IdleEvent::Playlist => UiEvent::Playlist,
            IdleEvent::Options => UiEvent::Options,
            IdleEvent::Database => UiEvent::Database,
            IdleEvent::StoredPlaylist => UiEvent::StoredPlaylist,
            _ => return Err(()),
        })
    }
}

pub fn restore_terminal<B: Backend + std::io::Write>(terminal: &mut Terminal<B>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(terminal.show_cursor()?)
}

pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    let mut stdout = std::io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.clear()?;
    Ok(terminal)
}

#[derive(Debug)]
enum KeyHandleResultInternal {
    /// Action warrants a render
    RenderRequested,
    /// Action does NOT warrant a render
    SkipRender,
    /// Event was not handled and should bubble up
    KeyNotHandled,
    /// Display a modal
    Modal(Option<Box<dyn Modal>>),
}

pub enum KeyHandleResult {
    /// Action warrants a render
    RenderRequested,
    /// Action does NOT warrant a render
    SkipRender,
    /// Exit the application
    Quit,
}

impl From<KeyHandleResultInternal> for KeyHandleResult {
    fn from(value: KeyHandleResultInternal) -> Self {
        match value {
            KeyHandleResultInternal::SkipRender => KeyHandleResult::SkipRender,
            _ => KeyHandleResult::RenderRequested,
        }
    }
}

impl From<&Level> for Color {
    fn from(value: &Level) -> Self {
        match value {
            Level::Info => Color::Blue,
            Level::Warn => Color::Yellow,
            Level::Error => Color::Red,
            Level::Debug => Color::LightGreen,
            Level::Trace => Color::Magenta,
        }
    }
}

trait DurationExt {
    fn to_string(&self) -> String;
}

impl DurationExt for Duration {
    fn to_string(&self) -> String {
        let secs = self.as_secs();
        let min = secs / 60;
        format!("{}:{:0>2}", min, secs - min * 60)
    }
}

impl From<&FilterKind> for &'static str {
    fn from(value: &FilterKind) -> Self {
        match value {
            FilterKind::Exact => "Exact match",
            FilterKind::Contains => "Contains value",
            FilterKind::StartsWith => "Starts with value",
            FilterKind::Regex => "Regex",
        }
    }
}

impl std::fmt::Display for FilterKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FilterKind::Exact => write!(f, "Exact match"),
            FilterKind::Contains => write!(f, "Contains value"),
            FilterKind::StartsWith => write!(f, "Starts with value"),
            FilterKind::Regex => write!(f, "Regex"),
        }
    }
}

impl FilterKind {
    fn cycle(&mut self) -> &mut Self {
        *self = match self {
            FilterKind::Exact => FilterKind::Contains,
            FilterKind::Contains => FilterKind::StartsWith,
            FilterKind::StartsWith => FilterKind::Regex,
            FilterKind::Regex => FilterKind::Exact,
        };
        self
    }
}

impl Config {
    fn as_header_table_block(&self) -> ratatui::widgets::Block {
        if !self.theme.draw_borders {
            return ratatui::widgets::Block::default();
        }
        Block::default().border_style(self.as_border_style())
    }

    fn as_tabs_block(&self) -> ratatui::widgets::Block {
        if !self.theme.draw_borders {
            return ratatui::widgets::Block::default()/* .padding(Padding::new(0, 0, 1, 1)) */;
        }

        ratatui::widgets::Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .border_set(border::ONE_EIGHTH_WIDE)
            .border_style(self.as_border_style())
    }

    fn as_border_style(&self) -> ratatui::style::Style {
        self.theme.borders_style
    }

    fn as_text_style(&self) -> ratatui::style::Style {
        self.theme
            .text_color
            .map(|color| Style::default().fg(color))
            .unwrap_or_default()
    }

    fn as_styled_progress_bar(&self) -> widgets::progress_bar::ProgressBar {
        let progress_bar_colors = &self.theme.progress_bar;
        widgets::progress_bar::ProgressBar::default()
            .thumb_style(progress_bar_colors.thumb_style)
            .track_style(progress_bar_colors.track_style)
            .elapsed_style(progress_bar_colors.elapsed_style)
            .elapsed_char(self.theme.progress_bar.symbols[0])
            .thumb_char(self.theme.progress_bar.symbols[1])
            .track_char(self.theme.progress_bar.symbols[2])
    }

    fn as_styled_scrollbar(&self) -> ratatui::widgets::Scrollbar {
        ratatui::widgets::Scrollbar::default()
            .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight)
            .track_symbol(Some(self.theme.scrollbar.symbols[0]))
            .thumb_symbol(self.theme.scrollbar.symbols[1])
            .begin_symbol(Some(self.theme.scrollbar.symbols[2]))
            .end_symbol(Some(self.theme.scrollbar.symbols[3]))
            .track_style(self.theme.scrollbar.track_style)
            .begin_style(self.theme.scrollbar.ends_style)
            .end_style(self.theme.scrollbar.ends_style)
            .thumb_style(self.theme.scrollbar.thumb_style)
    }
}
