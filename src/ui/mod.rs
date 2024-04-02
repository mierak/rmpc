use std::{io::Stdout, ops::AddAssign, time::Duration};

use anyhow::Result;
use crossterm::{
    event::KeyEvent,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::{Backend, Constraint, CrosstermBackend, Layout},
    style::{Color, Style},
    symbols::border,
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use strum::Display;

use crate::{
    config::Config,
    mpd::{
        client::Client,
        commands::{volume::Bound, State as MpdState},
        mpd_client::{FilterKind, MpdClient},
    },
};
use crate::{mpd::version::Version, state::State};

#[cfg(debug_assertions)]
use self::screens::logs::LogsScreen;
use self::{
    modals::{Modal, Modals},
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

#[derive(Debug, Default)]
pub struct SharedUiState {
    status_message: Option<StatusMessage>,
    frame_counter: u32,
}

#[derive(Debug)]
pub struct Ui<'a> {
    client: Client<'a>,
    screens: Screens,
    shared_state: SharedUiState,
    active_modal: Option<Modals>,
}

impl<'a> Ui<'a> {
    pub fn new(client: Client<'a>, config: &Config) -> Ui<'a> {
        Self {
            client,
            screens: Screens::new(config),
            shared_state: SharedUiState::default(),
            active_modal: None,
        }
    }
}

#[derive(Debug, Default)]
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
    fn new(config: &Config) -> Self {
        Self {
            queue: QueueScreen::new(config),
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
    ($self:ident, $app:ident, $fn:ident($($param:expr),+)) => {
        match $app.active_tab {
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

impl Ui<'_> {
    pub fn render(&mut self, frame: &mut Frame, app: &mut crate::state::State) -> Result<()> {
        if let Some(bg_color) = app.config.ui.background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), frame.size());
        }
        self.shared_state.frame_counter.add_assign(1);
        if self
            .shared_state
            .status_message
            .as_ref()
            .is_some_and(|m| m.created.elapsed() > std::time::Duration::from_secs(5))
        {
            self.shared_state.status_message = None;
        }

        let [header_area, content_area, bar_area] = *Layout::vertical([
            Constraint::Length(if app.config.ui.draw_borders { 5 } else { 3 }),
            Constraint::Percentage(100),
            Constraint::Min(1),
        ])
        .split(frame.size()) else {
            return Ok(());
        };

        let header = Header::new(app.config, app.active_tab, &app.status).set_song(app.current_song.as_ref());

        frame.render_widget(header, header_area);

        if let Some(StatusMessage { message, level, .. }) = &self.shared_state.status_message {
            let status_bar = Paragraph::new(message.to_owned())
                .alignment(ratatui::prelude::Alignment::Center)
                .style(Style::default().fg(level.into()).bg(Color::Black));
            frame.render_widget(status_bar, bar_area);
        } else if app.config.status_update_interval_ms.is_some() {
            let elapsed_bar = app.config.as_styled_progress_bar();
            let elapsed_bar = if app.status.duration == Duration::ZERO {
                elapsed_bar.value(0.0)
            } else {
                elapsed_bar.value(app.status.elapsed.as_secs_f32() / app.status.duration.as_secs_f32())
            };
            frame.render_widget(elapsed_bar, bar_area);
        }

        #[cfg(debug_assertions)]
        frame.render_widget(
            Paragraph::new(format!("{} frames", self.shared_state.frame_counter)),
            bar_area,
        );

        if app.config.ui.draw_borders {
            screen_call!(self, app, render(frame, content_area, app, &mut self.shared_state))?;
        } else {
            screen_call!(
                self,
                app,
                render(
                    frame,
                    ratatui::prelude::Rect {
                        x: content_area.x,
                        y: content_area.y + 1,
                        width: content_area.width,
                        height: content_area.height - 1,
                    },
                    app,
                    &mut self.shared_state
                )
            )?;
        }

        if let Some(ref mut modal) = self.active_modal {
            Self::render_modal(modal, frame, app, &mut self.shared_state)?;
        }

        Ok(())
    }

    fn render_modal(
        active_modal: &mut modals::Modals,
        frame: &mut Frame<'_>,
        app: &mut State,
        shared: &mut SharedUiState,
    ) -> Result<()> {
        match active_modal {
            modals::Modals::ConfirmQueueClear(ref mut m) => m.render(frame, app, shared),
            modals::Modals::SaveQueue(ref mut m) => m.render(frame, app, shared),
            modals::Modals::RenamePlaylist(ref mut m) => m.render(frame, app, shared),
            modals::Modals::AddToPlaylist(ref mut m) => m.render(frame, app, shared),
        }
    }
    fn handle_modal_key(
        active_modal: &mut modals::Modals,
        client: &mut Client<'_>,
        key: KeyEvent,
        app: &mut State,
        shared: &mut SharedUiState,
    ) -> Result<KeyHandleResultInternal> {
        match active_modal {
            modals::Modals::ConfirmQueueClear(ref mut m) => m.handle_key(key, client, app, shared),
            modals::Modals::SaveQueue(ref mut m) => m.handle_key(key, client, app, shared),
            modals::Modals::RenamePlaylist(ref mut m) => m.handle_key(key, client, app, shared),
            modals::Modals::AddToPlaylist(ref mut m) => m.handle_key(key, client, app, shared),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, app: &mut State) -> Result<KeyHandleResult> {
        macro_rules! screen_call_inner {
            ($fn:ident($($param:expr),+)) => {
                screen_call!(self, app, $fn($($param),+))?
            }
        }
        if let Some(ref mut modal) = self.active_modal {
            return match Self::handle_modal_key(modal, &mut self.client, key, app, &mut self.shared_state)? {
                KeyHandleResultInternal::Modal(None) => {
                    self.active_modal = None;
                    screen_call_inner!(refresh(&mut self.client, app, &mut self.shared_state));
                    Ok(KeyHandleResult::RenderRequested)
                }
                r => Ok(r.into()),
            };
        }

        match screen_call_inner!(handle_action(key, &mut self.client, app, &mut self.shared_state)) {
            KeyHandleResultInternal::RenderRequested => return Ok(KeyHandleResult::RenderRequested),
            KeyHandleResultInternal::SkipRender => return Ok(KeyHandleResult::SkipRender),
            KeyHandleResultInternal::Modal(modal) => {
                self.active_modal = modal;
                return Ok(KeyHandleResult::RenderRequested);
            }
            KeyHandleResultInternal::KeyNotHandled => {
                if let Some(action) = app.config.keybinds.global.get(&key.into()) {
                    match action {
                        GlobalAction::NextTrack if app.status.state == MpdState::Play => self.client.next()?,
                        GlobalAction::PreviousTrack if app.status.state == MpdState::Play => self.client.prev()?,
                        GlobalAction::Stop if app.status.state == MpdState::Play => self.client.stop()?,
                        GlobalAction::ToggleRepeat => self.client.repeat(!app.status.repeat)?,
                        GlobalAction::ToggleSingle => self.client.single(app.status.single.cycle())?,
                        GlobalAction::ToggleRandom => self.client.random(!app.status.random)?,
                        GlobalAction::ToggleConsume if self.client.version < Version::new(0, 24, 0) => {
                            self.client.consume(app.status.consume.cycle_pre_mpd_24())?;
                        }
                        GlobalAction::ToggleConsume => {
                            self.client.consume(app.status.consume.cycle())?;
                        }
                        GlobalAction::TogglePause
                            if app.status.state == MpdState::Play || app.status.state == MpdState::Pause =>
                        {
                            self.client.pause_toggle()?;
                            return Ok(KeyHandleResult::SkipRender);
                        }
                        GlobalAction::TogglePause => {}
                        GlobalAction::VolumeUp => {
                            self.client
                                .set_volume(app.status.volume.inc_by(app.config.volume_step))?;
                        }
                        GlobalAction::VolumeDown => {
                            self.client
                                .set_volume(app.status.volume.dec_by(app.config.volume_step))?;
                        }
                        GlobalAction::SeekForward if app.status.state == MpdState::Play => {
                            self.client.seek_curr_forwards(5)?;
                        }
                        GlobalAction::SeekBack if app.status.state == MpdState::Play => {
                            self.client.seek_curr_backwards(5)?;
                        }
                        GlobalAction::NextTab => {
                            screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                            app.active_tab = app.active_tab.next();
                            screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::PreviousTab => {
                            screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                            app.active_tab = app.active_tab.prev();
                            screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::QueueTab if !matches!(app.active_tab, screens::Screens::Queue) => {
                            screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                            app.active_tab = screens::Screens::Queue;
                            screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::DirectoriesTab if !matches!(app.active_tab, screens::Screens::Directories) => {
                            screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                            app.active_tab = screens::Screens::Directories;
                            screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::ArtistsTab if !matches!(app.active_tab, screens::Screens::Artists) => {
                            screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                            app.active_tab = screens::Screens::Artists;
                            screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::AlbumsTab if !matches!(app.active_tab, screens::Screens::Albums) => {
                            screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                            app.active_tab = screens::Screens::Albums;
                            screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::PlaylistsTab if !matches!(app.active_tab, screens::Screens::Playlists) => {
                            screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                            app.active_tab = screens::Screens::Playlists;
                            screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::SearchTab if !matches!(app.active_tab, screens::Screens::Search) => {
                            screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                            app.active_tab = screens::Screens::Search;
                            screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));
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
                    }
                    Ok(KeyHandleResult::SkipRender)
                } else {
                    Ok(KeyHandleResult::SkipRender)
                }
            }
        }
    }

    pub fn before_show(&mut self, app: &mut State) -> Result<()> {
        screen_call!(self, app, before_show(&mut self.client, app, &mut self.shared_state))
    }

    pub fn display_message(&mut self, message: String, level: Level) {
        self.shared_state.status_message = Some(StatusMessage {
            message,
            level,
            created: std::time::Instant::now(),
        });
    }
}

#[derive(Debug, Display, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, Clone, Copy)]
pub enum GlobalAction {
    Quit,
    NextTrack,
    PreviousTrack,
    Stop,
    ToggleRepeat,
    ToggleSingle,
    ToggleRandom,
    ToggleConsume,
    TogglePause,
    VolumeUp,
    VolumeDown,
    SeekForward,
    SeekBack,
    NextTab,
    PreviousTab,
    QueueTab,
    DirectoriesTab,
    ArtistsTab,
    AlbumsTab,
    PlaylistsTab,
    SearchTab,
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

enum KeyHandleResultInternal {
    /// Action warrants a render
    RenderRequested,
    /// Action does NOT warrant a render
    SkipRender,
    /// Event was not handled and should bubble up
    KeyNotHandled,
    /// Display a modal
    Modal(Option<Modals>),
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

trait BoolExt {
    fn to_onoff(&self) -> &'static str;
}

impl BoolExt for bool {
    fn to_onoff(&self) -> &'static str {
        if *self {
            "On"
        } else {
            "Off"
        }
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
        if !self.ui.draw_borders {
            return ratatui::widgets::Block::default();
        }
        Block::default().border_style(self.as_border_style())
    }

    fn as_tabs_block(&self) -> ratatui::widgets::Block {
        // if !self.ui.borders {
        //     return ratatui::widgets::Block::default().padding(Padding::new(0, 0, 1, 1));
        // }
        if !self.ui.draw_borders {
            return ratatui::widgets::Block::default()/* .padding(Padding::new(0, 0, 1, 1)) */;
        }

        ratatui::widgets::Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .border_set(border::ONE_EIGHTH_WIDE)
            .border_style(self.as_border_style())
    }

    fn as_border_style(&self) -> ratatui::style::Style {
        self.ui.borders_style
    }

    fn as_styled_progress_bar(&self) -> widgets::progress_bar::ProgressBar {
        let progress_bar_colors = &self.ui.progress_bar;
        widgets::progress_bar::ProgressBar::default()
            .thumb_style(progress_bar_colors.thumb_style)
            .track_style(progress_bar_colors.track_style)
            .elapsed_style(progress_bar_colors.elapsed_style)
            .elapsed_char(self.ui.progress_bar.symbols[0])
            .thumb_char(self.ui.progress_bar.symbols[1])
            .track_char(self.ui.progress_bar.symbols[2])
    }

    fn as_styled_scrollbar(&self) -> ratatui::widgets::Scrollbar {
        ratatui::widgets::Scrollbar::default()
            .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight)
            .track_symbol(Some(self.ui.scrollbar.symbols[0]))
            .thumb_symbol(self.ui.scrollbar.symbols[1])
            .begin_symbol(Some(self.ui.scrollbar.symbols[2]))
            .end_symbol(Some(self.ui.scrollbar.symbols[3]))
            .track_style(self.ui.scrollbar.track_style)
            .begin_style(self.ui.scrollbar.ends_style)
            .end_style(self.ui.scrollbar.ends_style)
            .thumb_style(self.ui.scrollbar.thumb_style)
    }
}
