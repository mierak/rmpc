use ansi_to_tui::IntoText;
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::{Alignment, Backend, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, ScrollbarOrientation},
    Frame,
};

use crate::{
    mpd::{client::Client, errors::MpdError},
    state::State,
    ui::{
        widgets::scrollbar::{Scrollbar, ScrollbarState},
        Render, SharedUiState,
    },
};

use super::Screen;

#[derive(Debug, Default)]
pub struct LogsScreen {
    scrollbar: ScrollbarState,
}

#[async_trait]
impl Screen for LogsScreen {
    fn render<B: Backend>(
        &mut self,
        frame: &mut Frame<B>,
        area: Rect,
        app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> anyhow::Result<()> {
        let lines = app
            .logs
            .0
            .iter()
            .flat_map(|l| l.into_text().unwrap().lines)
            .enumerate()
            .map(|(idx, mut l)| {
                if idx == self.scrollbar.get_position() as usize {
                    l.patch_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
                }
                l
            })
            .collect::<Vec<Line>>();

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
        self.scrollbar
            .content_length(TryInto::<u16>::try_into(lines.len()).unwrap());
        self.scrollbar.viewport_content_length(content.height.saturating_sub(2));

        let logs_wg = Paragraph::new(lines[self.scrollbar.get_range_usize()].to_vec())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Gray))
                    .title(Span::styled(
                        format!("Logs: {}", lines.len()),
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
            )
            .alignment(Alignment::Left);

        frame.render_widget(logs_wg, content);
        frame.render_stateful_widget(
            scrollbar,
            scroll.inner(&Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut self.scrollbar.inner,
        );

        Ok(())
    }

    async fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
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
                self.scrollbar.next();
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('k') => {
                self.scrollbar.prev();
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('d') => {
                for _ in 0..5 {
                    self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Forward);
                }
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('u') => {
                for _ in 0..5 {
                    self.scrollbar.scroll(ratatui::widgets::ScrollDirection::Backward);
                }
                return Ok(Render::NoSkip);
            }
            _ => {}
        }
        Ok(Render::Skip)
    }
}
