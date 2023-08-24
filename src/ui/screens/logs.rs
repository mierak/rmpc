use ansi_to_tui::IntoText;
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::{Backend, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Span,
    widgets::{Block, Borders, List, ListItem, ListState, Scrollbar, ScrollbarOrientation},
    Frame,
};

use crate::{
    mpd::{client::Client, errors::MpdError},
    state::State,
    ui::{MyState, Render, SharedUiState},
};

use super::Screen;

#[derive(Debug, Default)]
pub struct LogsScreen {
    scrolling_state: MyState<ListState>,
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
                if self.scrolling_state.inner.selected().is_some_and(|v| v == idx) {
                    l.patch_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
                }
                ListItem::new(l)
            })
            .collect::<Vec<ListItem>>();

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .track_symbol("│")
            .end_symbol(Some("↓"))
            .track_style(Style::default().fg(Color::White).bg(Color::Black))
            .begin_style(Style::default().fg(Color::White).bg(Color::Black))
            .end_style(Style::default().fg(Color::White).bg(Color::Black))
            .thumb_style(Style::default().fg(Color::Blue));

        let [content, scroll] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
               Constraint::Percentage(100),
               Constraint::Min(0),
        ].as_ref())
            .split(area) else {
                return Ok(())
            };

        let content_len = lines.len();
        self.scrolling_state.content_len(Some(content_len as u16));
        self.scrolling_state.viewport_len(Some(content.height));

        let logs_wg = List::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Gray))
                .title(Span::styled(
                    format!("Logs: {}", content_len),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
        );

        frame.render_stateful_widget(logs_wg, content, &mut self.scrolling_state.inner);
        frame.render_stateful_widget(
            scrollbar,
            scroll.inner(&Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut self.scrolling_state.scrollbar_state,
        );

        Ok(())
    }

    async fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        // self.list_state.last();
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
                self.scrolling_state.next();
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('k') => {
                self.scrolling_state.prev();
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('d') => {
                for _ in 0..5 {
                    self.scrolling_state.next();
                }
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('u') => {
                for _ in 0..5 {
                    self.scrolling_state.prev();
                }
                return Ok(Render::NoSkip);
            }
            _ => {}
        }
        Ok(Render::Skip)
    }
}
