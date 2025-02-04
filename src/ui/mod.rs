use std::{collections::HashMap, io::Stdout};

use anyhow::{Context, Result, anyhow};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use itertools::Itertools;
use modals::{
    decoders::DecodersModal,
    input_modal::InputModal,
    keybinds::KeybindsModal,
    outputs::OutputsModal,
    song_info::SongInfoModal,
};
use panes::{PaneContainer, Panes, pane_call};
use ratatui::{
    Frame,
    Terminal,
    layout::Rect,
    prelude::{Backend, CrosstermBackend},
    style::{Color, Style},
    symbols::border,
    widgets::{Block, Borders},
};
use tab_screen::TabScreen;

use self::{modals::Modal, panes::Pane};
use crate::{
    MpdQueryResult,
    config::{
        Config,
        cli::Args,
        keys::GlobalAction,
        tabs::{PaneType, SizedPaneOrSplit, TabName},
    },
    context::AppContext,
    core::command::{create_env, run_external},
    mpd::{
        commands::{State, idle::IdleEvent},
        mpd_client::{FilterKind, MpdClient, ValueChange},
        version::Version,
    },
    shared::{
        events::{Level, WorkRequest},
        key_event::KeyEvent,
        macros::{modal, status_error, status_info, status_warn},
        mouse_event::MouseEvent,
    },
};

pub mod browser;
pub mod dirstack;
pub mod image;
pub mod modals;
pub mod panes;
pub mod tab_screen;
pub mod widgets;

#[derive(Debug)]
pub struct StatusMessage {
    pub message: String,
    pub level: Level,
    pub created: std::time::Instant,
}

#[derive(Debug)]
pub struct Ui<'ui> {
    panes: PaneContainer<'ui>,
    modals: Vec<Box<dyn Modal>>,
    active_tab: TabName,
    tabs: HashMap<TabName, TabScreen>,
    layout: SizedPaneOrSplit,
    area: Rect,
}

const OPEN_DECODERS_MODAL: &str = "open_decoders_modal";
const OPEN_OUTPUTS_MODAL: &str = "open_outputs_modal";

macro_rules! active_tab_call {
    ($self:ident, $fn:ident($($param:expr),+)) => {
        $self.tabs
            .get_mut(&$self.active_tab)
            .context(anyhow!("Expected tab '{}' to be defined. Please report this along with your config.", $self.active_tab))?
            .$fn(&mut $self.panes, $($param),+)
    }
}

impl<'ui> Ui<'ui> {
    pub fn new(context: &AppContext) -> Result<Ui<'ui>> {
        let active_tab = *context.config.tabs.names.first().context("Expected at least one tab")?;
        Ok(Self {
            active_tab,
            panes: PaneContainer::new(context)?,
            layout: context.config.theme.layout.clone(),
            modals: Vec::default(),
            tabs: context
                .config
                .tabs
                .tabs
                .iter()
                .map(|(name, screen)| -> Result<_> {
                    Ok((*name, TabScreen::new(screen.panes.clone())))
                })
                .try_collect()?,
            area: Rect::default(),
        })
    }

    fn calc_areas(&mut self, area: Rect, _context: &AppContext) {
        self.area = area;
    }

    fn change_tab(&mut self, new_tab: TabName, context: &AppContext) -> Result<()> {
        self.layout.for_each_pane(self.area, &mut |pane, _, _, _| {
            match self.panes.get_mut(&pane.pane) {
                Panes::TabContent => {
                    active_tab_call!(self, on_hide(context))?;
                }
                _ => {}
            };
            Ok(())
        })?;

        self.active_tab = new_tab;
        self.on_event(UiEvent::TabChanged(new_tab), context)?;

        self.layout.for_each_pane(self.area, &mut |pane, pane_area, _, _| {
            match self.panes.get_mut(&pane.pane) {
                Panes::TabContent => {
                    active_tab_call!(self, before_show(pane_area, context))?;
                }
                _ => {}
            };
            Ok(())
        })
    }

    pub fn render(&mut self, frame: &mut Frame, context: &mut AppContext) -> Result<()> {
        self.area = frame.area();
        if let Some(bg_color) = context.config.theme.background_color {
            frame
                .render_widget(Block::default().style(Style::default().bg(bg_color)), frame.area());
        }

        self.layout.for_each_pane_custom_data(
            self.area,
            &mut *frame,
            &mut |pane, pane_area, block, block_area, frame| {
                match self.panes.get_mut(&pane.pane) {
                    Panes::TabContent => {
                        active_tab_call!(self, render(frame, pane_area, context))?;
                    }
                    mut pane_instance => {
                        pane_call!(pane_instance, render(frame, pane_area, context))?;
                    }
                };
                frame.render_widget(
                    block.border_style(context.config.as_border_style()),
                    block_area,
                );
                Ok(())
            },
            &mut |block, block_area, frame| {
                frame.render_widget(
                    block.border_style(context.config.as_border_style()),
                    block_area,
                );
                Ok(())
            },
        )?;

        for modal in &mut self.modals {
            modal.render(frame, context)?;
        }

        Ok(())
    }

    pub fn handle_mouse_event(
        &mut self,
        event: MouseEvent,
        context: &mut AppContext,
    ) -> Result<()> {
        if let Some(ref mut modal) = self.modals.last_mut() {
            modal.handle_mouse_event(event, context)?;
            return Ok(());
        }

        self.layout.for_each_pane(self.area, &mut |pane, _, _, _| {
            match self.panes.get_mut(&pane.pane) {
                Panes::TabContent => {
                    active_tab_call!(self, handle_mouse_event(event, context))?;
                }
                mut pane_instance => {
                    pane_call!(pane_instance, handle_mouse_event(event, context))?;
                }
            };
            Ok(())
        })
    }

    pub fn handle_key(
        &mut self,
        key: &mut KeyEvent,
        context: &mut AppContext,
    ) -> Result<KeyHandleResult> {
        if let Some(ref mut modal) = self.modals.last_mut() {
            modal.handle_key(key, context)?;
            return Ok(KeyHandleResult::None);
        }

        active_tab_call!(self, handle_action(key, context))?;

        if let Some(action) = key.as_global_action(context) {
            match action {
                GlobalAction::Command { command, .. } => {
                    let cmd = command.parse();
                    log::debug!("executing {:?}", cmd);

                    if let Ok(Args { command: Some(cmd), .. }) = cmd {
                        if context.work_sender.send(WorkRequest::Command(cmd)).is_err() {
                            log::error!("Failed to send command");
                        }
                    }
                }
                GlobalAction::CommandMode => {
                    modal!(
                        context,
                        InputModal::new(context)
                            .title("Execute a command")
                            .confirm_label("Execute")
                            .on_confirm(|context, value| {
                                let cmd = value.parse();
                                log::debug!("executing {:?}", cmd);

                                if let Ok(Args { command: Some(cmd), .. }) = cmd {
                                    if context.work_sender.send(WorkRequest::Command(cmd)).is_err()
                                    {
                                        log::error!("Failed to send command");
                                    }
                                };
                                Ok(())
                            })
                    );
                }
                GlobalAction::NextTrack if context.status.state == State::Play => {
                    context.command(move |client| {
                        client.next()?;
                        Ok(())
                    });
                }
                GlobalAction::PreviousTrack if context.status.state == State::Play => {
                    context.command(move |client| {
                        client.prev()?;
                        Ok(())
                    });
                }
                GlobalAction::Stop
                    if matches!(context.status.state, State::Play | State::Pause) =>
                {
                    context.command(move |client| {
                        client.stop()?;
                        Ok(())
                    });
                }
                GlobalAction::ToggleRepeat => {
                    let repeat = !context.status.repeat;
                    context.command(move |client| {
                        client.repeat(repeat)?;
                        Ok(())
                    });
                }
                GlobalAction::ToggleRandom => {
                    let random = !context.status.random;
                    context.command(move |client| {
                        client.random(random)?;
                        Ok(())
                    });
                }
                GlobalAction::ToggleSingle => {
                    let single = context.status.single;
                    context.command(move |client| {
                        if client.version() < Version::new(0, 21, 0) {
                            client.single(single.cycle_pre_mpd_24())?;
                        } else {
                            client.single(single.cycle())?;
                        }
                        Ok(())
                    });
                }
                GlobalAction::ToggleConsume => {
                    let consume = context.status.consume;
                    context.command(move |client| {
                        if client.version() < Version::new(0, 24, 0) {
                            client.consume(consume.cycle_pre_mpd_24())?;
                        } else {
                            client.consume(consume.cycle())?;
                        }
                        Ok(())
                    });
                }
                GlobalAction::TogglePause
                    if matches!(context.status.state, State::Play | State::Pause) =>
                {
                    context.command(move |client| {
                        client.pause_toggle()?;
                        Ok(())
                    });
                }
                GlobalAction::TogglePause => {}
                GlobalAction::VolumeUp => {
                    let step = context.config.volume_step;
                    context.command(move |client| {
                        client.volume(ValueChange::Increase(step.into()))?;
                        Ok(())
                    });
                }
                GlobalAction::VolumeDown => {
                    let step = context.config.volume_step;
                    context.command(move |client| {
                        client.volume(ValueChange::Decrease(step.into()))?;
                        Ok(())
                    });
                }
                GlobalAction::SeekForward
                    if matches!(context.status.state, State::Play | State::Pause) =>
                {
                    context.command(move |client| {
                        client.seek_current(ValueChange::Increase(5))?;
                        Ok(())
                    });
                }
                GlobalAction::SeekBack
                    if matches!(context.status.state, State::Play | State::Pause) =>
                {
                    context.command(move |client| {
                        client.seek_current(ValueChange::Decrease(5))?;
                        Ok(())
                    });
                }
                GlobalAction::NextTab => {
                    self.change_tab(context.config.next_screen(self.active_tab), context)?;
                    context.render()?;
                }
                GlobalAction::PreviousTab => {
                    self.change_tab(context.config.prev_screen(self.active_tab), context)?;
                    context.render()?;
                }
                GlobalAction::SwitchToTab(name) => {
                    if context.config.tabs.names.contains(&name) {
                        self.change_tab(name, context)?;
                        context.render()?;
                    } else {
                        status_error!(
                            "Tab with name '{}' does not exist. Check your configuration.",
                            name
                        );
                    }
                }
                GlobalAction::NextTrack => {}
                GlobalAction::PreviousTrack => {}
                GlobalAction::Stop => {}
                GlobalAction::SeekBack => {}
                GlobalAction::SeekForward => {}
                GlobalAction::ExternalCommand { command, .. } => {
                    run_external(command, create_env(context, std::iter::empty::<&str>()));
                }
                GlobalAction::Quit => return Ok(KeyHandleResult::Quit),
                GlobalAction::ShowHelp => {
                    let modal = KeybindsModal::new(context);
                    modal!(context, modal);
                }
                GlobalAction::ShowOutputs => {
                    context
                        .query()
                        .id(OPEN_OUTPUTS_MODAL)
                        .replace_id(OPEN_OUTPUTS_MODAL)
                        .query(|client| Ok(MpdQueryResult::Outputs(client.outputs()?.0)));
                }
                GlobalAction::ShowDecoders => {
                    context
                        .query()
                        .id(OPEN_DECODERS_MODAL)
                        .replace_id(OPEN_DECODERS_MODAL)
                        .query(|client| Ok(MpdQueryResult::Decoders(client.decoders()?.0)));
                }
                GlobalAction::ShowCurrentSongInfo => {
                    if let Some((_, current_song)) = context.find_current_song_in_queue() {
                        modal!(context, SongInfoModal::new(current_song.clone()));
                    } else {
                        status_info!("No song is currently playing");
                    }
                }
            }
        };

        Ok(KeyHandleResult::None)
    }

    pub fn before_show(&mut self, area: Rect, context: &mut AppContext) -> Result<()> {
        self.calc_areas(area, context);

        self.layout.for_each_pane(self.area, &mut |pane, pane_area, _, _| {
            match self.panes.get_mut(&pane.pane) {
                Panes::TabContent => {
                    active_tab_call!(self, before_show(pane_area, context))?;
                }
                mut pane_instance => {
                    pane_call!(pane_instance, calculate_areas(pane_area, context))?;
                    pane_call!(pane_instance, before_show(context))?;
                }
            };
            Ok(())
        })
    }

    pub fn on_ui_app_event(&mut self, event: UiAppEvent, context: &mut AppContext) -> Result<()> {
        match event {
            UiAppEvent::Modal(modal) => {
                self.modals.push(modal.0);
                self.on_event(UiEvent::ModalOpened, context)?;
                context.render()?;
            }
            UiAppEvent::PopModal => {
                self.modals.pop();
                self.on_event(UiEvent::ModalClosed, context)?;
                context.render()?;
            }
            UiAppEvent::ChangeTab(tab_name) => self.change_tab(tab_name, context)?,
        }
        Ok(())
    }

    pub fn resize(&mut self, area: Rect, context: &AppContext) -> Result<()> {
        log::trace!(area:?; "Terminal was resized");
        self.calc_areas(area, context);

        self.layout.for_each_pane(self.area, &mut |pane, pane_area, _, _| {
            match self.panes.get_mut(&pane.pane) {
                Panes::TabContent => {
                    active_tab_call!(self, resize(pane_area, context))?;
                }
                mut pane_instance => {
                    pane_call!(pane_instance, calculate_areas(pane_area, context))?;
                    pane_call!(pane_instance, resize(pane_area, context))?;
                }
            };
            Ok(())
        })
    }

    pub fn on_event(&mut self, mut event: UiEvent, context: &AppContext) -> Result<()> {
        match event {
            UiEvent::Database => {
                status_warn!(
                    "The music database has been updated. Some parts of the UI may have been reinitialized to prevent inconsistent behaviours."
                );
            }
            _ => {}
        }

        let contains_pane = |p| {
            self.tabs
                .get(&self.active_tab)
                .is_some_and(|tab| tab.panes.panes_iter().any(|pane| pane.pane == p))
                || self.layout.panes_iter().any(|pane| pane.pane == p)
        };

        for name in context.config.active_panes {
            match self.panes.get_mut(name) {
                #[cfg(debug_assertions)]
                Panes::Logs(p) => p.on_event(&mut event, contains_pane(PaneType::Logs), context),
                Panes::Queue(p) => p.on_event(&mut event, contains_pane(PaneType::Queue), context),
                Panes::Directories(p) => {
                    p.on_event(&mut event, contains_pane(PaneType::Directories), context)
                }
                Panes::Albums(p) => {
                    p.on_event(&mut event, contains_pane(PaneType::Albums), context)
                }
                Panes::Artists(p) => {
                    p.on_event(&mut event, contains_pane(PaneType::Artists), context)
                }
                Panes::Playlists(p) => {
                    p.on_event(&mut event, contains_pane(PaneType::Playlists), context)
                }
                Panes::Search(p) => {
                    p.on_event(&mut event, contains_pane(PaneType::Search), context)
                }
                Panes::AlbumArtists(p) => {
                    p.on_event(&mut event, contains_pane(PaneType::AlbumArtists), context)
                }
                Panes::AlbumArt(p) => {
                    p.on_event(&mut event, contains_pane(PaneType::AlbumArt), context)
                }
                Panes::Lyrics(p) => {
                    p.on_event(&mut event, contains_pane(PaneType::Lyrics), context)
                }
                Panes::ProgressBar(p) => {
                    p.on_event(&mut event, contains_pane(PaneType::ProgressBar), context)
                }
                Panes::Header(p) => {
                    p.on_event(&mut event, contains_pane(PaneType::Header), context)
                }
                Panes::Tabs(p) => p.on_event(&mut event, contains_pane(PaneType::Tabs), context),
                Panes::TabContent => Ok(()),
                #[cfg(debug_assertions)]
                Panes::FrameCount(p) => {
                    p.on_event(&mut event, contains_pane(PaneType::Tabs), context)
                }
            }?;
        }

        Ok(())
    }

    pub(crate) fn on_command_finished(
        &mut self,
        id: &'static str,
        pane: Option<PaneType>,
        data: MpdQueryResult,
        context: &mut AppContext,
    ) -> Result<()> {
        let contains_pane = |p| {
            self.tabs
                .get(&self.active_tab)
                .is_some_and(|tab| tab.panes.panes_iter().any(|pane| pane.pane == p))
                || self.layout.panes_iter().any(|pane| pane.pane == p)
        };
        match pane {
            Some(pane) => match self.panes.get_mut(&pane) {
                #[cfg(debug_assertions)]
                Panes::Logs(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::Logs), context)
                }
                Panes::Queue(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::Queue), context)
                }
                Panes::Directories(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::Directories), context)
                }
                Panes::Albums(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::Albums), context)
                }
                Panes::Artists(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::Artists), context)
                }
                Panes::Playlists(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::Playlists), context)
                }
                Panes::Search(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::Search), context)
                }
                Panes::AlbumArtists(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::AlbumArtists), context)
                }
                Panes::AlbumArt(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::AlbumArt), context)
                }
                Panes::Lyrics(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::Lyrics), context)
                }
                Panes::ProgressBar(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::ProgressBar), context)
                }
                Panes::Header(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::Header), context)
                }
                Panes::Tabs(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::Tabs), context)
                }
                Panes::TabContent => Ok(()),
                #[cfg(debug_assertions)]
                Panes::FrameCount(p) => {
                    p.on_query_finished(id, data, contains_pane(PaneType::FrameCount), context)
                }
            }?,
            None => match (id, data) {
                (OPEN_OUTPUTS_MODAL, MpdQueryResult::Outputs(outputs)) => {
                    modal!(context, OutputsModal::new(outputs));
                }
                (OPEN_DECODERS_MODAL, MpdQueryResult::Decoders(decoders)) => {
                    modal!(context, DecodersModal::new(decoders));
                }
                (id, mut data) => {
                    // TODO a proper modal target
                    for modal in &mut self.modals {
                        modal.on_query_finished(id, &mut data, context)?;
                    }
                }
            },
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct ModalWrapper(Box<dyn Modal + Send + Sync>);

#[derive(Debug)]
pub enum UiAppEvent {
    Modal(ModalWrapper),
    PopModal,
    ChangeTab(TabName),
}

#[derive(Debug, Eq, Hash, PartialEq)]
#[allow(dead_code)]
pub enum UiEvent {
    Player,
    Database,
    StoredPlaylist,
    LogAdded(Vec<u8>),
    ModalOpened,
    ModalClosed,
    Exit,
    LyricsIndexed,
    SongChanged,
    Reconnected,
    Status(String, Level),
    TabChanged(TabName),
}

impl TryFrom<IdleEvent> for UiEvent {
    type Error = ();

    fn try_from(event: IdleEvent) -> Result<Self, ()> {
        Ok(match event {
            IdleEvent::Player => UiEvent::Player,
            IdleEvent::Database => UiEvent::Database,
            IdleEvent::StoredPlaylist => UiEvent::StoredPlaylist,
            _ => return Err(()),
        })
    }
}

pub fn restore_terminal<B: Backend + std::io::Write>(
    terminal: &mut Terminal<B>,
    enable_mouse: bool,
) -> Result<()> {
    if enable_mouse {
        execute!(std::io::stdout(), DisableMouseCapture)?;
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(terminal.show_cursor()?)
}

pub fn setup_terminal(enable_mouse: bool) -> Result<Terminal<CrosstermBackend<Stdout>>> {
    let mut stdout = std::io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    if enable_mouse {
        execute!(stdout, EnableMouseCapture)?;
    }
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.clear()?;
    Ok(terminal)
}

pub enum KeyHandleResult {
    None,
    Quit,
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
            .and_then(|(idx, _)| {
                names.get((if idx == 0 { names.len() - 1 } else { idx - 1 }) % names.len())
            })
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
        self.theme.text_color.map(|color| Style::default().fg(color)).unwrap_or_default()
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
        let symbols = self.theme.scrollbar.symbols;
        let track = if symbols[0].is_empty() { None } else { Some(symbols[0]) };
        let begin = if symbols[2].is_empty() { None } else { Some(symbols[2]) };
        let end = if symbols[3].is_empty() { None } else { Some(symbols[3]) };
        ratatui::widgets::Scrollbar::default()
            .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight)
            .track_symbol(track)
            .thumb_symbol(self.theme.scrollbar.symbols[1])
            .begin_symbol(begin)
            .end_symbol(end)
            .track_style(self.theme.scrollbar.track_style)
            .begin_style(self.theme.scrollbar.ends_style)
            .end_style(self.theme.scrollbar.ends_style)
            .thumb_style(self.theme.scrollbar.thumb_style)
    }
}
