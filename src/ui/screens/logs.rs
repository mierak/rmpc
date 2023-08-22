use std::io::Stdout;

use ansi_to_tui::IntoText;
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::{Alignment, Constraint, CrosstermBackend, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::{
    mpd::{client::Client, errors::MpdError},
    state::State,
    ui::{Render, SharedUiState},
};

use super::Screen;

#[derive(Debug, Default)]
pub struct LogsScreen {
    pub scrollbar: ScrollbarState,
    pub scrollbar_position: usize,
}

#[async_trait]
impl Screen for LogsScreen {
    fn render(
        &mut self,
        frame: &mut Frame<CrosstermBackend<Stdout>>,
        area: Rect,
        app: &crate::state::State,
        _shared: &SharedUiState,
    ) -> anyhow::Result<()> {
        let lines = app
            .logs
            .0
            .iter()
            .flat_map(|l| l.into_text().unwrap().lines)
            .collect::<Vec<Line>>();
        let len = lines.len();

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let [content, scroll] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
               Constraint::Percentage(100),
               Constraint::Min(0),
        ].as_ref())
            .split(area) else {
                return Ok(())
            };
        self.scrollbar = self.scrollbar.content_length(TryInto::<u16>::try_into(len).unwrap());
        self.scrollbar = self.scrollbar.viewport_content_length(content.height);

        let logs_wg = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Gray))
                    .title(Span::styled(
                        format!("Logs: {}", len),
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
            )
            .alignment(Alignment::Left)
            .scroll((
                std::convert::TryInto::<u16>::try_into(self.scrollbar_position)
                    .unwrap()
                    .saturating_sub(content.height),
                0,
            ));
        // .wrap(Wrap { trim: true });

        frame.render_widget(logs_wg, content);
        frame.render_stateful_widget(
            scrollbar,
            scroll.inner(&Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut self.scrollbar,
        );

        Ok(())
    }

    async fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        self.scrollbar_position = _app.logs.0.len();
        self.scrollbar.last();
        Ok(())
    }

    async fn handle_key(
        &mut self,
        key: KeyEvent,
        _client: &mut Client<'_>,
        _app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<Render, MpdError> {
        match key.code {
            KeyCode::Char('j') => {
                self.scrollbar_position = self.scrollbar_position.saturating_add(1);
                self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Forward);
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('k') => {
                self.scrollbar_position = self.scrollbar_position.saturating_sub(1);
                self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Backward);
                return Ok(Render::NoSkip);
            }
            // TODO
            KeyCode::Char('d') => {
                self.scrollbar_position = self.scrollbar_position.saturating_add(5);
                self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Forward);
                self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Forward);
                self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Forward);
                self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Forward);
                self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Forward);
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('u') => {
                self.scrollbar_position = self.scrollbar_position.saturating_sub(5);
                self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Backward);
                self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Backward);
                self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Backward);
                self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Backward);
                self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Backward);
                return Ok(Render::NoSkip);
            }
            _ => {}
        }
        Ok(Render::Skip)
    }
}
