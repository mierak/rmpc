use std::{collections::HashMap, io::Stdout, ops::AddAssign, time::Duration};

use anyhow::{anyhow, Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, MouseButton, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use enum_map::{enum_map, Enum, EnumMap};
use itertools::Itertools;
use modals::{keybinds::KeybindsModal, outputs::OutputsModal};
use panes::{PaneContainer, Panes};
use ratatui::{
    layout::Rect,
    prelude::{Backend, Constraint, CrosstermBackend, Layout},
    style::{Color, Style},
    symbols::border,
    text::Text,
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use tab_screen::TabScreen;
use widgets::app_tabs::AppTabs;

use crate::{
    cli::{create_env, run_external},
    config::{
        cli::Args,
        keys::{CommonAction, GlobalAction},
        tabs::TabName,
        Config,
    },
    mpd::{
        client::Client,
        commands::{idle::IdleEvent, volume::Bound, Song, State as MpdState},
        mpd_client::{FilterKind, MpdClient, ValueChange},
    },
    utils::{
        macros::{status_error, try_ret},
        mouse_event::MouseEvent,
    },
};
use crate::{context::AppContext, mpd::version::Version};

use self::{modals::Modal, panes::Pane, widgets::header::Header};

pub mod image;
pub mod modals;
pub mod panes;
pub mod tab_screen;
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
    panes: PaneContainer,
    modals: Vec<Box<dyn Modal>>,
    status_message: Option<StatusMessage>,
    rendered_frames_count: u32,
    current_song: Option<Song>,
    command: Option<String>,
    active_tab: TabName,
    tabs: HashMap<TabName, TabScreen>,
    areas: EnumMap<Areas, Rect>,
}

impl Ui {
    pub fn new(context: &AppContext) -> Result<Ui> {
        Ok(Self {
            panes: PaneContainer::new(context),
            status_message: None,
            rendered_frames_count: 0,
            current_song: None,
            modals: Vec::default(),
            command: None,
            active_tab: *context.config.tabs.names.first().context("Expected at least one tab")?,
            tabs: context
                .config
                .tabs
                .tabs
                .iter()
                .map(|(name, screen)| -> Result<_> { Ok((*name, TabScreen::new(&screen.panes))) })
                .try_collect()?,
            areas: enum_map! {
                _ => Rect::default()
            },
        })
    }

    fn calc_areas(&mut self, area: Rect, context: &AppContext) -> Result<()> {
        let tab_area_height = match (context.config.theme.tab_bar.enabled, context.config.theme.draw_borders) {
            (true, true) => 3,
            (true, false) => 1,
            (false, _) => 0,
        };

        let [header_area, tabs_area, content_area, bar_area] = *Layout::vertical([
            Constraint::Length(u16::try_from(context.config.theme.header.rows.len())?),
            Constraint::Length(tab_area_height), // Tab bar
            Constraint::Percentage(100),
            Constraint::Min(1),
        ])
        .split(area) else {
            return Ok(());
        };
        self.areas[Areas::Header] = header_area;
        self.areas[Areas::Tabs] = tabs_area;
        self.areas[Areas::Content] = content_area;
        self.areas[Areas::Bar] = bar_area;

        Ok(())
    }
}

macro_rules! screen_call {
    ($self:ident, $fn:ident($($param:expr),+)) => {
        $self.tabs
            .get_mut(&$self.active_tab)
            .context(anyhow!("Expected tab '{}' to be defined. Please report this along with your config.", $self.active_tab))?
            .$fn(&mut $self.panes, $($param),+)
    }
}

#[derive(Debug, Enum)]
enum Areas {
    Header,
    Tabs,
    Content,
    Bar,
}

impl Ui {
    pub fn post_render(&mut self, frame: &mut Frame, context: &mut AppContext) -> Result<()> {
        screen_call!(self, post_render(frame, context))
    }

    pub fn render(&mut self, frame: &mut Frame, context: &mut AppContext) -> Result<()> {
        self.calc_areas(frame.area(), context)?;

        if let Some(bg_color) = context.config.theme.background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), frame.area());
        }
        self.rendered_frames_count.add_assign(1);
        if self
            .status_message
            .as_ref()
            .is_some_and(|m| m.created.elapsed() > std::time::Duration::from_secs(5))
        {
            self.status_message = None;
        }

        let header = Header::new(context.config, &context.status, self.current_song.as_ref());
        frame.render_widget(header, self.areas[Areas::Header]);

        if self.areas[Areas::Tabs].height > 0 {
            let app_tabs = AppTabs::new(self.active_tab, context.config);
            frame.render_widget(app_tabs, self.areas[Areas::Tabs]);
        }

        if let Some(command) = &self.command {
            let [leader_area, command_area] =
                *Layout::horizontal([Constraint::Length(1), Constraint::Percentage(100)]).split(self.areas[Areas::Bar])
            else {
                return Ok(());
            };

            let status_bar = Paragraph::new(command.as_str())
                .alignment(ratatui::prelude::Alignment::Left)
                .style(context.config.as_text_style());

            frame.render_widget(Text::from(":"), leader_area); // TODO: use key from config
            frame.render_widget(status_bar, command_area);
        } else if let Some(StatusMessage { message, level, .. }) = &self.status_message {
            let status_bar = Paragraph::new(message.to_owned())
                .alignment(ratatui::prelude::Alignment::Center)
                .style(Style::default().fg(level.into()).bg(Color::Black));
            frame.render_widget(status_bar, self.areas[Areas::Bar]);
        } else if context.config.status_update_interval_ms.is_some() {
            let elapsed_bar = context.config.as_styled_progress_bar();
            let elapsed_bar = if context.status.duration == Duration::ZERO {
                elapsed_bar.value(0.0)
            } else {
                elapsed_bar.value(context.status.elapsed.as_secs_f32() / context.status.duration.as_secs_f32())
            };
            frame.render_widget(elapsed_bar, self.areas[Areas::Bar]);
        }

        #[cfg(debug_assertions)]
        frame.render_widget(
            Paragraph::new(format!("{} frames", self.rendered_frames_count)),
            self.areas[Areas::Bar],
        );

        let content_area = self.areas[Areas::Content];
        if context.config.theme.draw_borders {
            screen_call!(self, render(frame, content_area, context))?;
        } else {
            screen_call!(
                self,
                render(
                    frame,
                    ratatui::prelude::Rect {
                        x: content_area.x,
                        y: content_area.y,
                        width: content_area.width,
                        height: content_area.height,
                    },
                    context
                )
            )?;
        }

        for modal in &mut self.modals {
            modal.render(frame, context)?;
        }

        Ok(())
    }

    pub fn handle_mouse_event(
        &mut self,
        event: MouseEvent,
        client: &mut impl MpdClient,
        context: &mut AppContext,
    ) -> Result<KeyHandleResult> {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) if self.areas[Areas::Bar].contains(event.into()) => {
                let second_to_seek_to = context
                    .status
                    .duration
                    .mul_f32(f32::from(event.x) / f32::from(self.areas[Areas::Bar].width))
                    .as_secs();
                client.seek_current(ValueChange::Set(u32::try_from(second_to_seek_to)?))?;

                return Ok(KeyHandleResult::RenderRequested);
            }
            MouseEventKind::Down(_mouse_button) => {}
            MouseEventKind::Up(_mouse_button) => {}
            MouseEventKind::Drag(_mouse_button) => {}
            MouseEventKind::Moved => {}
            MouseEventKind::ScrollDown => {}
            MouseEventKind::ScrollUp => {}
            MouseEventKind::ScrollLeft => {}
            MouseEventKind::ScrollRight => {}
        }

        Ok(KeyHandleResult::RenderRequested)
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        context: &mut AppContext,
        client: &mut Client<'_>,
    ) -> Result<KeyHandleResult> {
        let action = context.config.keybinds.navigation.get(&key.into());
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
                        cmd.execute(client, context.config, |request, _| {
                            if let Err(err) = context.work_sender.send(request) {
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

        if let Some(ref mut modal) = self.modals.last_mut() {
            return match modal.handle_key(key, client, context)? {
                KeyHandleResultInternal::Modal(None) => {
                    self.modals.pop();
                    self.on_event(UiEvent::ModalClosed, context, client)?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                r => Ok(r.into()),
            };
        }

        match screen_call!(self, handle_action(key, client, context))? {
            KeyHandleResultInternal::RenderRequested => return Ok(KeyHandleResult::RenderRequested),
            KeyHandleResultInternal::FullRenderRequested => return Ok(KeyHandleResult::FullRenderRequested),
            KeyHandleResultInternal::SkipRender => return Ok(KeyHandleResult::SkipRender),
            KeyHandleResultInternal::Modal(Some(modal)) => {
                self.modals.push(modal);
                self.on_event(UiEvent::ModalOpened, context, client)?;
                return Ok(KeyHandleResult::RenderRequested);
            }
            KeyHandleResultInternal::Modal(None) => {
                self.modals.pop();
                self.on_event(UiEvent::ModalClosed, context, client)?;
                return Ok(KeyHandleResult::RenderRequested);
            }
            KeyHandleResultInternal::KeyNotHandled => {
                if let Some(action) = context.config.keybinds.global.get(&key.into()) {
                    match action {
                        GlobalAction::Command { command, .. } => {
                            let cmd = command.parse();
                            log::debug!("executing {:?}", cmd);

                            self.command = None;
                            if let Ok(Args { command: Some(cmd), .. }) = cmd {
                                cmd.execute(client, context.config, |request, _| {
                                    if let Err(err) = context.work_sender.send(request) {
                                        status_error!("Failed to send work request: {}", err);
                                    }
                                })?;
                            }
                        }
                        GlobalAction::CommandMode => {
                            self.command = Some(String::new());
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::NextTrack if context.status.state == MpdState::Play => client.next()?,
                        GlobalAction::PreviousTrack if context.status.state == MpdState::Play => client.prev()?,
                        GlobalAction::Stop if context.status.state == MpdState::Play => client.stop()?,
                        GlobalAction::ToggleRepeat => client.repeat(!context.status.repeat)?,
                        GlobalAction::ToggleRandom => client.random(!context.status.random)?,
                        GlobalAction::ToggleSingle if client.version() < Version::new(0, 21, 0) => {
                            client.single(context.status.single.cycle_pre_mpd_24())?;
                        }
                        GlobalAction::ToggleSingle => client.single(context.status.single.cycle())?,
                        GlobalAction::ToggleConsume if client.version() < Version::new(0, 24, 0) => {
                            client.consume(context.status.consume.cycle_pre_mpd_24())?;
                        }
                        GlobalAction::ToggleConsume => {
                            client.consume(context.status.consume.cycle())?;
                        }
                        GlobalAction::TogglePause
                            if context.status.state == MpdState::Play || context.status.state == MpdState::Pause =>
                        {
                            client.pause_toggle()?;
                            return Ok(KeyHandleResult::SkipRender);
                        }
                        GlobalAction::TogglePause => {}
                        GlobalAction::VolumeUp => {
                            client.set_volume(*context.status.volume.inc_by(context.config.volume_step))?;
                        }
                        GlobalAction::VolumeDown => {
                            client.set_volume(*context.status.volume.dec_by(context.config.volume_step))?;
                        }
                        GlobalAction::SeekForward if context.status.state == MpdState::Play => {
                            client.seek_current(ValueChange::Increase(5))?;
                        }
                        GlobalAction::SeekBack if context.status.state == MpdState::Play => {
                            client.seek_current(ValueChange::Decrease(5))?;
                        }
                        GlobalAction::NextTab => {
                            screen_call!(self, on_hide(client, &context))?;
                            self.active_tab = context.config.next_screen(self.active_tab);
                            screen_call!(self, before_show(client, &context))?;
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::PreviousTab => {
                            screen_call!(self, on_hide(client, &context))?;
                            self.active_tab = context.config.prev_screen(self.active_tab);
                            screen_call!(self, before_show(client, &context))?;
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::SwitchToTab(name) => {
                            if context.config.tabs.names.contains(name) {
                                screen_call!(self, on_hide(client, &context))?;
                                self.active_tab = *name;
                                screen_call!(self, before_show(client, &context))?;
                            } else {
                                status_error!("Tab with name '{}' does not exist. Check your configuration.", name);
                            }
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::NextTrack => {}
                        GlobalAction::PreviousTrack => {}
                        GlobalAction::Stop => {}
                        GlobalAction::SeekBack => {}
                        GlobalAction::SeekForward => {}
                        GlobalAction::ExternalCommand { command, .. } => {
                            run_external(command, create_env(context, std::iter::empty::<&str>(), client)?);
                        }
                        GlobalAction::Quit => return Ok(KeyHandleResult::Quit),
                        GlobalAction::ShowHelp => {
                            self.modals.push(Box::new(KeybindsModal::new(context)));
                            self.on_event(UiEvent::ModalOpened, context, client)?;
                            return Ok(KeyHandleResult::RenderRequested);
                        }
                        GlobalAction::ShowOutputs => {
                            self.modals.push(Box::new(OutputsModal::new(client.outputs()?.0)));
                            self.on_event(UiEvent::ModalOpened, context, client)?;
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

    pub fn before_show(&mut self, context: &mut AppContext, client: &mut impl MpdClient) -> Result<()> {
        self.current_song = try_ret!(client.get_current_song(), "Failed get current song");
        screen_call!(self, before_show(client, &context))
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
        context: &mut AppContext,
        client: &mut impl MpdClient,
    ) -> Result<KeyHandleResult> {
        match event {
            UiEvent::Player => {
                self.current_song = try_ret!(client.get_current_song(), "Failed get current song");
            }
            UiEvent::Playlist => {}
            UiEvent::Database => {}
            UiEvent::StoredPlaylist => {}
            UiEvent::LogAdded(_) =>
            {
                #[cfg(debug_assertions)]
                if self.active_tab == "Logs".into() {
                    return Ok(KeyHandleResult::RenderRequested);
                }
            }
            UiEvent::Update => {}
            UiEvent::Resized { .. } => {}
            UiEvent::ModalOpened => {}
            UiEvent::ModalClosed => {}
            UiEvent::Mixer => {}
            UiEvent::Options => {}
            UiEvent::Exit => {}
        }

        let mut ret = KeyHandleResultInternal::SkipRender;

        for name in context.config.tabs.active_panes {
            let result = match self.panes.get_mut(*name) {
                #[cfg(debug_assertions)]
                Panes::Logs(p) => p.on_event(&mut event, client, context),
                Panes::Queue(p) => p.on_event(&mut event, client, context),
                Panes::Directories(p) => p.on_event(&mut event, client, context),
                Panes::Albums(p) => p.on_event(&mut event, client, context),
                Panes::Artists(p) => p.on_event(&mut event, client, context),
                Panes::Playlists(p) => p.on_event(&mut event, client, context),
                Panes::Search(p) => p.on_event(&mut event, client, context),
                Panes::AlbumArtists(p) => p.on_event(&mut event, client, context),
                Panes::AlbumArt(p) => p.on_event(&mut event, client, context),
            };

            match self.handle_screen_event_result(result)? {
                KeyHandleResult::RenderRequested => ret = KeyHandleResultInternal::RenderRequested,
                KeyHandleResult::FullRenderRequested => ret = KeyHandleResultInternal::FullRenderRequested,
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
            Ok(KeyHandleResultInternal::FullRenderRequested) => Ok(KeyHandleResult::FullRenderRequested),
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
    Resized { columns: u16, rows: u16 },
    ModalOpened,
    ModalClosed,
    Exit,
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
    execute!(std::io::stdout(), DisableMouseCapture)?;
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(terminal.show_cursor()?)
}

pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    let mut stdout = std::io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    execute!(stdout, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.clear()?;
    Ok(terminal)
}

#[derive(Debug)]
enum KeyHandleResultInternal {
    /// Action warrants a render
    RenderRequested,
    /// Action warrants a render with whole window clear
    FullRenderRequested,
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
    /// Action warrants a render with whole window clear
    FullRenderRequested,
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
    fn next_screen(&self, current_screen: TabName) -> TabName {
        let names = self.tabs.names;
        *names
            .iter()
            .enumerate()
            .find(|(_, s)| **s == current_screen)
            .and_then(|(idx, _)| names.get((idx + 1) % names.len()))
            .unwrap_or(&current_screen)
    }

    fn prev_screen(&self, current_screen: TabName) -> TabName {
        let names = self.tabs.names;
        *names
            .iter()
            .enumerate()
            .find(|(_, s)| **s == current_screen)
            .and_then(|(idx, _)| names.get((if idx == 0 { names.len() - 1 } else { idx - 1 }) % names.len()))
            .unwrap_or(&current_screen)
    }

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

    fn as_focused_border_style(&self) -> ratatui::style::Style {
        self.theme.highlight_border_style
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
