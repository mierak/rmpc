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

#[derive(Debug, Default)]
pub struct SharedUiState {}

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

impl Ui<'_> {
    pub fn render(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        app: &crate::state::State,
    ) -> Result<()> {
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

            if app.error.is_empty() {
                let elapsed_bar = ProgressBar::default().fg(Color::Blue).bg(Color::Black);
                let elapsed_bar = if app.status.duration == Duration::ZERO {
                    elapsed_bar.value(0.0)
                } else {
                    elapsed_bar.value(app.status.elapsed.as_secs_f32() / app.status.duration.as_secs_f32())
                };
                frame.render_widget(elapsed_bar, bar_area);
            } else {
                let status_bar = Paragraph::new(app.error.into_text().unwrap())
                    .style(Style::default().fg(Color::Red).bg(Color::Black));
                frame.render_widget(status_bar, bar_area);
            }
            frame.render_widget(tabs, tabs_area);

            match app.active_tab {
                screens::Screens::Queue => self
                    .screens
                    .queue
                    .render(frame, content, app, &self.shared_state)
                    .unwrap(),
                screens::Screens::Logs => self
                    .screens
                    .logs
                    .render(frame, content, app, &self.shared_state)
                    .unwrap(),
                screens::Screens::Directories => self
                    .screens
                    .directories
                    .render(frame, content, app, &self.shared_state)
                    .unwrap(),
            };
        })?;

        Ok(())
    }

    #[instrument(skip(self, app), fields(screen))]
    pub async fn handle_key(&mut self, key: KeyEvent, app: &mut State) -> Result<Render, MpdError> {
        match key.code {
            KeyCode::Right => {
                match app.active_tab {
                    screens::Screens::Queue => {
                        self.screens
                            .queue
                            .on_hide(&mut self.client, app, &mut self.shared_state)
                            .await
                    }
                    screens::Screens::Logs => {
                        self.screens
                            .logs
                            .on_hide(&mut self.client, app, &mut self.shared_state)
                            .await
                    }
                    screens::Screens::Directories => {
                        self.screens
                            .directories
                            .on_hide(&mut self.client, app, &mut self.shared_state)
                            .await
                    }
                }
                .unwrap();
                app.active_tab = app.active_tab.next();
                tracing::Span::current().record("screen", app.active_tab.to_string());
                match app.active_tab {
                    screens::Screens::Queue => {
                        self.screens
                            .queue
                            .before_show(&mut self.client, app, &mut self.shared_state)
                            .await
                    }
                    screens::Screens::Logs => {
                        self.screens
                            .logs
                            .before_show(&mut self.client, app, &mut self.shared_state)
                            .await
                    }
                    screens::Screens::Directories => {
                        self.screens
                            .directories
                            .before_show(&mut self.client, app, &mut self.shared_state)
                            .await
                    }
                }
                .unwrap();
                Ok(Render::NoSkip)
            }
            KeyCode::Left => {
                match app.active_tab {
                    screens::Screens::Queue => {
                        self.screens
                            .queue
                            .on_hide(&mut self.client, app, &mut self.shared_state)
                            .await
                    }
                    screens::Screens::Logs => {
                        self.screens
                            .logs
                            .on_hide(&mut self.client, app, &mut self.shared_state)
                            .await
                    }
                    screens::Screens::Directories => {
                        self.screens
                            .directories
                            .on_hide(&mut self.client, app, &mut self.shared_state)
                            .await
                    }
                }
                .unwrap();
                app.active_tab = app.active_tab.prev();
                tracing::Span::current().record("screen", app.active_tab.to_string());
                match app.active_tab {
                    screens::Screens::Queue => {
                        self.screens
                            .queue
                            .before_show(&mut self.client, app, &mut self.shared_state)
                            .await
                    }
                    screens::Screens::Logs => {
                        self.screens
                            .logs
                            .before_show(&mut self.client, app, &mut self.shared_state)
                            .await
                    }
                    screens::Screens::Directories => {
                        self.screens
                            .directories
                            .before_show(&mut self.client, app, &mut self.shared_state)
                            .await
                    }
                }
                .unwrap();
                Ok(Render::NoSkip)
            }
            _ => {
                tracing::Span::current().record("screen", app.active_tab.to_string());
                match app.active_tab {
                    screens::Screens::Queue => self.screens.queue.handle_key(key, &mut self.client, app).await,
                    screens::Screens::Logs => self.screens.logs.handle_key(key, &mut self.client, app).await,
                    screens::Screens::Directories => {
                        self.screens.directories.handle_key(key, &mut self.client, app).await
                    }
                }
            }
        }
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
