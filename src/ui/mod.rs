use std::{io::Stdout, time::Duration};

use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::{
    event::{KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::{Constraint, CrosstermBackend, Direction, Layout},
    style::{Color, Style},
    widgets::Paragraph,
    Terminal,
};
use strum::{IntoEnumIterator, VariantNames};
use tracing::instrument;

use crate::state::State;
use crate::{
    mpd::{client::Client, errors::MpdError},
    ui::widgets::tabs::Tabs,
};

use self::{
    screens::{directories::DirectoriesScreen, logs::LogsScreen, queue::QueueScreen, Screen},
    widgets::progress_bar::ProgressBar,
};

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
}

#[derive(Debug)]
pub struct Ui<'a> {
    client: Client<'a>,
    screens: Screens,
    shared_state: SharedUiState,
}

impl<'a> Ui<'a> {
    pub fn new(client: Client<'a>) -> Ui<'a> {
        Self {
            client,
            screens: Screens::default(),
            shared_state: SharedUiState::default(),
        }
    }
}

#[derive(Debug, Default)]
struct Screens {
    queue: QueueScreen,
    logs: LogsScreen,
    directories: DirectoriesScreen,
}

macro_rules! do_for_screen {
    ($screen:expr, $fn:ident, $($param:expr),+) => {
        $screen.$fn($($param),+)
    };
}

macro_rules! screen_call {
    ($self:ident, $app:ident, $fn:ident($($param:expr),+)) => {
        match $app.active_tab {
            screens::Screens::Queue => do_for_screen!($self.screens.queue, $fn, $($param),+),
            screens::Screens::Logs => do_for_screen!($self.screens.logs, $fn, $($param),+),
            screens::Screens::Directories => do_for_screen!($self.screens.directories, $fn, $($param),+),
        }
    }
}

impl Ui<'_> {
    pub fn render(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        app: &mut crate::state::State,
    ) -> Result<()> {
        if self
            .shared_state
            .status_message
            .as_ref()
            .is_some_and(|m| m.created.elapsed() > std::time::Duration::from_secs(5))
        {
            self.shared_state.status_message = None;
        }
        terminal.draw(|frame| {
            let tab_names = screens::Screens::VARIANTS
                .iter()
                .enumerate()
                .map(|(i, e)| format!("{: ^17}", format!("({}) {e}", i + 1)))
                .collect::<Vec<String>>();
            let tabs = Tabs::new(tab_names)
                .select(
                    screens::Screens::iter()
                        .enumerate()
                        .find(|(_, t)| t == &app.active_tab)
                        .unwrap()
                        .0,
                )
                .divider("|")
                .block(ratatui::widgets::Block::default().borders(ratatui::widgets::Borders::ALL))
                .highlight_style(Style::default().fg(Color::Black).bg(Color::Blue));

            let [tabs_area, content, bar_area] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                   Constraint::Min(3),
                   Constraint::Percentage(100),
                   Constraint::Min(1),
            ].as_ref())
                .split(frame.size()) else {
                    return
                };

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
            frame.render_widget(tabs, tabs_area);

            screen_call!(self, app, render(frame, content, app, &mut self.shared_state)).unwrap();
        })?;

        Ok(())
    }

    #[instrument(skip(self, app), fields(screen))]
    pub async fn handle_key(&mut self, key: KeyEvent, app: &mut State) -> Result<Render, MpdError> {
        macro_rules! screen_call_inner {
            ($fn:ident($($param:expr),+)) => {
                screen_call!(self, app, $fn($($param),+)).await.unwrap();
            }
        }
        match key.code {
            KeyCode::Right => {
                screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                app.active_tab = app.active_tab.next();
                tracing::Span::current().record("screen", app.active_tab.to_string());
                screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));

                Ok(Render::NoSkip)
            }
            KeyCode::Left => {
                screen_call_inner!(on_hide(&mut self.client, app, &mut self.shared_state));

                app.active_tab = app.active_tab.prev();
                tracing::Span::current().record("screen", app.active_tab.to_string());
                screen_call_inner!(before_show(&mut self.client, app, &mut self.shared_state));

                Ok(Render::NoSkip)
            }
            _ => {
                tracing::Span::current().record("screen", app.active_tab.to_string());
                screen_call_inner!(handle_key(key, &mut self.client, app, &mut self.shared_state));
                Ok(Render::NoSkip)
            }
        }
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
