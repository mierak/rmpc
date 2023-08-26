use std::{io::Stdout, time::Duration};

use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::{Alignment, Backend, Constraint, CrosstermBackend, Direction, Layout},
    style::{Color, Style, Stylize},
    widgets::{Borders, ListState, Paragraph, ScrollbarState, TableState},
    Frame, Terminal,
};
use strum::{IntoEnumIterator, VariantNames};
use tracing::instrument;

use crate::{
    mpd::commands::{volume::Bound, State as MpdState},
    mpd::{client::Client, errors::MpdError},
    ui::widgets::tabs::Tabs,
};
use crate::{state::State, ui::widgets::line::Line};

use self::{
    modals::{confirm_queue_clear::ConfirmQueueClearModal, Modal},
    screens::{
        albums::AlbumsScreen, artists::ArtistsScreen, directories::DirectoriesScreen, logs::LogsScreen,
        queue::QueueScreen, Screen,
    },
    widgets::{frame_counter::FrameCounter, progress_bar::ProgressBar},
};

pub mod modals;
pub mod screens;
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
    pub created: tokio::time::Instant,
}

impl StatusMessage {
    pub fn new(message: String, level: Level) -> Self {
        Self {
            message,
            level,
            created: tokio::time::Instant::now(),
        }
    }
}

#[derive(Debug, Default)]
pub struct SharedUiState {
    pub status_message: Option<StatusMessage>,
    pub frame_counter: FrameCounter,
}

#[derive(Debug)]
pub struct Ui<'a> {
    client: Client<'a>,
    screens: Screens,
    modals: UiModals,
    shared_state: SharedUiState,
}

impl<'a> Ui<'a> {
    pub fn new(client: Client<'a>) -> Ui<'a> {
        Self {
            client,
            screens: Screens::default(),
            modals: UiModals::default(),
            shared_state: SharedUiState::default(),
        }
    }
}

#[derive(Debug, Default)]
struct UiModals {
    confirm_queue_clear: ConfirmQueueClearModal,
}
#[derive(Debug, Default)]
struct Screens {
    queue: QueueScreen,
    logs: LogsScreen,
    directories: DirectoriesScreen,
    albums: AlbumsScreen,
    artists: ArtistsScreen,
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
            screens::Screens::Logs => invoke!($self.screens.logs, $fn, $($param),+),
            screens::Screens::Directories => invoke!($self.screens.directories, $fn, $($param),+),
            screens::Screens::Artists => invoke!($self.screens.artists, $fn, $($param),+),
            screens::Screens::Albums => invoke!($self.screens.albums, $fn, $($param),+),
        }
    }
}

macro_rules! modal_call {
    ($self:ident, $app:ident, $fn:ident($($param:expr),+)) => {
        // todo unwrap
        match $app.visible_modal.as_ref().unwrap() {
            modals::Modals::ConfirmQueueClear => invoke!($self.modals.confirm_queue_clear, $fn, $($param),+),
        }
    }
}

impl Ui<'_> {
    pub fn render<B: Backend>(&mut self, frame: &mut Frame<B>, app: &mut crate::state::State) -> Result<()> {
        if self
            .shared_state
            .status_message
            .as_ref()
            .is_some_and(|m| m.created.elapsed() > std::time::Duration::from_secs(5))
        {
            self.shared_state.status_message = None;
        }
        let [title_area, tabs_area, content_area, bar_area] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Length(2), Constraint::Percentage(100), Constraint::Min(1)].as_ref())
            .split(frame.size()) else { return Ok(()) };

        let [title_left_area, title_ceter_area, title_right_area] = *Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    [
                        Constraint::Percentage(20),
                        Constraint::Percentage(60),
                        Constraint::Percentage(20),
                    ].as_ref(),
                ) .split(title_area) else { return Ok(()) };

        let [song_name_area, song_info_area] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(title_ceter_area.height/2), Constraint::Length(title_ceter_area.height/2)].as_ref())
            .split(title_ceter_area) else { return Ok(()) };

        let [volume_area, states_area] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(title_ceter_area.height/2), Constraint::Length(title_ceter_area.height/2)].as_ref())
            .split(title_right_area) else { return Ok(()) };

        let [status_area, elapsed_area] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(title_ceter_area.height/2), Constraint::Length(title_ceter_area.height/2)].as_ref())
            .split(title_left_area) else { return Ok(()) };

        let tab_names = screens::Screens::VARIANTS
            .iter()
            .map(|e| format!("{: ^13}", format!("{e}")))
            .collect::<Vec<String>>();

        let tabs = Tabs::new(tab_names)
            .select(
                screens::Screens::iter()
                    .enumerate()
                    .find(|(_, t)| t == &app.active_tab)
                    .unwrap()
                    .0,
            )
            .divider("")
            .block(ratatui::widgets::Block::default().borders(Borders::TOP))
            .highlight_style(Style::default().fg(Color::Black).bg(Color::Blue));

        // right
        let volume = crate::ui::widgets::volume::Volume::default()
            .value(*app.status.volume.value())
            .alignment(Alignment::Right)
            .style(Style::default().fg(Color::Blue));

        let on_style = Style::default().fg(Color::Gray);
        let off_style = Style::default().fg(Color::DarkGray);
        let states = Line::new(vec![
            (
                "Repeat".to_owned(),
                if app.status.repeat { on_style } else { off_style },
            ),
            (
                "Random".to_owned(),
                if app.status.random { on_style } else { off_style },
            ),
            match app.status.consume {
                crate::mpd::commands::status::OnOffOneshot::On => ("Consume".to_owned(), on_style),
                crate::mpd::commands::status::OnOffOneshot::Off => ("Consume".to_owned(), off_style),
                crate::mpd::commands::status::OnOffOneshot::Oneshot => ("Oneshot(C)".to_owned(), on_style),
            },
            match app.status.single {
                crate::mpd::commands::status::OnOffOneshot::On => ("Single".to_owned(), on_style),
                crate::mpd::commands::status::OnOffOneshot::Off => ("Single".to_owned(), off_style),
                crate::mpd::commands::status::OnOffOneshot::Oneshot => ("Oneshot(S)".to_owned(), on_style),
            },
        ])
        .separator(" / ".to_owned())
        .separator_style(on_style)
        .alignment(Alignment::Right);

        // center
        let song_name = Paragraph::new(app.current_song.as_ref().map_or("No song".to_owned(), |v| {
            v.title.as_ref().unwrap_or(&"No song".to_owned()).to_owned()
        }))
        .style(Style::default().bold())
        .alignment(Alignment::Center);

        // left
        let status = Paragraph::new(format!(
            "[{}] {} rendered frames",
            app.status.state, self.shared_state.frame_counter.frame_count
        ));
        let elapsed = Paragraph::new(format!(
            "{}/{}{}",
            app.status.elapsed.to_string(),
            app.status.duration.to_string(),
            app.status
                .bitrate
                .as_ref()
                .map_or("".to_owned(), |v| if v == "0" {
                    "".to_owned()
                } else {
                    format!(" ({} kbps)", v)
                })
                .to_owned()
        ))
        .style(Style::default().fg(Color::Gray));

        let song_info = app.current_song.as_ref().map_or(Line::default(), |v| {
            let artist = v.artist.as_ref().unwrap_or(&"Unknown".to_owned()).to_owned();
            let album = v.album.as_ref().unwrap_or(&"Unknown Album".to_owned()).to_owned();
            Line::new(vec![
                (artist, Style::default().fg(Color::Yellow)),
                (album, Style::default().fg(Color::LightBlue)),
            ])
            .alignment(Alignment::Center)
            .separator(" - ".to_owned())
            .separator_style(Style::default().bold())
        });

        if let Some(StatusMessage {
            ref message, ref level, ..
        }) = self.shared_state.status_message
        {
            let status_bar = Paragraph::new(message.into_text().unwrap())
                .alignment(ratatui::prelude::Alignment::Center)
                .style(Style::default().fg(level.to_color()).bg(Color::Black));
            frame.render_widget(status_bar, bar_area);
        } else {
            let elapsed_bar = ProgressBar::default().fg(Color::Blue).bg(Color::Black);
            let elapsed_bar = if app.status.duration == Duration::ZERO {
                elapsed_bar.value(0.0)
            } else {
                elapsed_bar.value(app.status.elapsed.as_secs_f32() / app.status.duration.as_secs_f32())
            };
            frame.render_widget(elapsed_bar, bar_area);
        }

        // fame.render_widget(&self.shared_state.frame_counter, left);
        frame.render_widget(states, states_area);
        frame.render_widget(status, status_area);
        frame.render_widget(elapsed, elapsed_area);
        frame.render_widget(volume, volume_area);
        frame.render_widget(song_name, song_name_area);
        frame.render_widget(song_info, song_info_area);
        frame.render_widget(tabs, tabs_area);

        screen_call!(self, app, render(frame, content_area, app, &mut self.shared_state)).unwrap();
        if app.visible_modal.is_some() {
            modal_call!(self, app, render(frame, app, &mut self.shared_state)).unwrap();
        }
        self.shared_state.frame_counter.increment();

        Ok(())
    }

    #[instrument(skip(self, app), fields(screen))]
    pub async fn handle_key(&mut self, key: KeyEvent, app: &mut State) -> Result<Render, MpdError> {
        macro_rules! screen_call_inner {
            ($fn:ident($($param:expr),+)) => {
                screen_call!(self, app, $fn($($param),+)).await.unwrap();
            }
        }
        if app.visible_modal.is_some() {
            return modal_call!(self, app, handle_key(key, &mut self.client, app)).await;
        } else {
            match key.code {
                // these two are here only to induce panic for testing
                KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => self.client.next().await?,
                KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => self.client.prev().await?,

                KeyCode::Char('n') if app.status.state == MpdState::Play => self.client.next().await?,
                KeyCode::Char('p') if app.status.state == MpdState::Play => self.client.prev().await?,
                KeyCode::Char('s') if app.status.state == MpdState::Play => self.client.stop().await?,
                KeyCode::Char('z') => self.client.repeat(!app.status.repeat).await?,
                KeyCode::Char('x') => self.client.random(!app.status.random).await?,
                KeyCode::Char('c') => self.client.single(app.status.single.cycle()).await?,
                KeyCode::Char('f') if app.status.state == MpdState::Play => self.client.seek_curr_forwards(5).await?,
                KeyCode::Char('b') if app.status.state == MpdState::Play => self.client.seek_curr_backwards(5).await?,
                KeyCode::Char(',') => self.client.set_volume(app.status.volume.dec()).await?,
                KeyCode::Char('.') => self.client.set_volume(app.status.volume.inc()).await?,
                KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                    app.active_tab = app.active_tab.next();
                    tracing::Span::current().record("screen", app.active_tab.to_string());
                    screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));

                    return Ok(Render::NoSkip);
                }
                KeyCode::Right => {
                    screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                    app.active_tab = app.active_tab.next();
                    tracing::Span::current().record("screen", app.active_tab.to_string());
                    screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));

                    return Ok(Render::NoSkip);
                }
                KeyCode::Left => {
                    screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                    app.active_tab = app.active_tab.prev();
                    tracing::Span::current().record("screen", app.active_tab.to_string());
                    screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));

                    return Ok(Render::NoSkip);
                }
                KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                    app.active_tab = app.active_tab.prev();
                    tracing::Span::current().record("screen", app.active_tab.to_string());
                    screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));

                    return Ok(Render::NoSkip);
                }
                _ => {
                    tracing::Span::current().record("screen", app.active_tab.to_string());
                    screen_call_inner!(handle_key(key, &mut self.client, app, &mut self.shared_state));
                    return Ok(Render::NoSkip);
                }
            }
        }
        Ok(Render::Skip)
    }

    pub async fn before_show(&mut self, app: &mut State) {
        screen_call!(self, app, before_show(&mut self.client, app, &mut self.shared_state))
            .await
            .unwrap();
    }

    pub fn display_message(&mut self, message: &str, level: Level) {
        self.shared_state.status_message = Some(StatusMessage {
            message: message.to_owned(),
            level,
            created: tokio::time::Instant::now(),
        })
    }
}

pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
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

/// NoSkip should be used only in rare cases when we do not receive idle event from mpd based on our action
/// as those idle events will trigger render by themselves.
/// These cases include selecting (not playing!) next/previous song
pub enum Render {
    Skip,
    NoSkip,
}

trait LevelExt {
    fn to_color(&self) -> Color;
}
impl LevelExt for Level {
    fn to_color(&self) -> Color {
        match *self {
            Level::Info => Color::Blue,
            Level::Warn => Color::Yellow,
            Level::Error => Color::Red,
            Level::Debug => Color::LightGreen,
            Level::Trace => Color::Magenta,
        }
    }
}

pub(self) trait DurationExt {
    fn to_string(&self) -> String;
}

impl DurationExt for Duration {
    fn to_string(&self) -> String {
        let secs = self.as_secs();
        let min = secs / 60;
        format!("{}:{:0>2}", min, secs - min * 60)
    }
}

pub(self) trait BoolExt {
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

#[derive(Debug, Default)]
struct MyState<T: ScrollingState> {
    pub scrollbar_state: ScrollbarState,
    pub inner: T,
    pub content_len: Option<u16>,
    pub viewport_len: Option<u16>,
}

impl<T: ScrollingState> MyState<T> {
    fn viewport_len(&mut self, viewport_len: Option<u16>) -> &Self {
        self.viewport_len = viewport_len;
        self.scrollbar_state = self.scrollbar_state.viewport_content_length(viewport_len.unwrap_or(0));
        self
    }

    fn content_len(&mut self, content_len: Option<u16>) -> &Self {
        self.content_len = content_len;
        self.scrollbar_state = self.scrollbar_state.content_length(content_len.unwrap_or(0));
        self
    }

    fn first(&mut self) {
        if self.content_len.is_some() {
            self.select(Some(0));
        } else {
            self.select(None);
        }
    }

    fn last(&mut self) {
        if let Some(item_count) = self.content_len {
            self.select(Some(item_count.saturating_sub(1) as usize));
        } else {
            self.select(None);
        }
    }

    fn next(&mut self) {
        if let Some(item_count) = self.content_len {
            let i = match self.get_selected() {
                Some(i) => {
                    if i >= item_count.saturating_sub(1) as usize {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            self.select(Some(i));
        } else {
            self.select(None);
        }
    }

    fn prev(&mut self) {
        if let Some(item_count) = self.content_len {
            let i = match self.get_selected() {
                Some(i) => {
                    if i == 0 {
                        item_count.saturating_sub(1) as usize
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            self.select(Some(i));
        } else {
            self.select(None);
        }
    }

    fn select(&mut self, idx: Option<usize>) {
        self.inner.select_scrolling(idx);
        self.scrollbar_state = self.scrollbar_state.position(idx.unwrap_or(0) as u16);
    }

    fn get_selected(&self) -> Option<usize> {
        self.inner.get_selected_scrolling()
    }
}

trait ScrollingState {
    fn select_scrolling(&mut self, idx: Option<usize>);
    fn get_selected_scrolling(&self) -> Option<usize>;
}

impl ScrollingState for TableState {
    fn select_scrolling(&mut self, idx: Option<usize>) {
        self.select(idx)
    }

    fn get_selected_scrolling(&self) -> Option<usize> {
        self.selected()
    }
}

impl ScrollingState for ListState {
    fn select_scrolling(&mut self, idx: Option<usize>) {
        self.select(idx)
    }

    fn get_selected_scrolling(&self) -> Option<usize> {
        self.selected()
    }
}
