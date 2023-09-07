use ansi_to_tui::IntoText;
use anyhow::Result;
use async_trait::async_trait;
use itertools::Itertools;
use ratatui::{
    prelude::{Backend, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Span,
    widgets::{Block, Borders, List, ListItem, ListState, Scrollbar, ScrollbarOrientation},
    Frame,
};

use crate::{
    mpd::client::Client,
    state::State,
    ui::{Render, SharedUiState},
};

use super::{dirstack::MyState, Screen};

#[derive(Debug, Default)]
pub struct LogsScreen {
    scrolling_state: MyState<ListState>,
}

#[async_trait]
impl Screen for LogsScreen {
    type Actions = LogsActions;
    fn render<B: Backend>(
        &mut self,
        frame: &mut Frame<B>,
        area: Rect,
        app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> anyhow::Result<()> {
        let lines: Vec<_> = app
            .logs
            .0
            .iter()
            .map(|l| -> Result<_> { Ok(l.into_text()?.lines) })
            .flatten_ok()
            .enumerate()
            .map(|(idx, l)| -> Result<_> {
                match l {
                    Ok(mut val) => {
                        if self.scrolling_state.inner.selected().is_some_and(|v| v == idx) {
                            val.patch_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
                        }
                        Ok(ListItem::new(val))
                    }
                    Err(err) => Err(err),
                }
            })
            .try_collect()?;

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .track_symbol(Some("│"))
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
        self.scrolling_state.content_len(Some(u16::try_from(content_len)?));
        self.scrolling_state.viewport_len(Some(content.height));

        let logs_wg = List::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Gray))
                .title(Span::styled(
                    format!("Logs: {content_len}"),
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
        self.scrolling_state.last();
        Ok(())
    }

    async fn handle_key(
        &mut self,
        action: Self::Actions,
        _client: &mut Client<'_>,
        _app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<Render> {
        match action {
            LogsActions::Down => self.scrolling_state.next(),
            LogsActions::Up => self.scrolling_state.prev(),
            LogsActions::DownHalf => self.scrolling_state.next_half_viewport(),
            LogsActions::UpHalf => self.scrolling_state.prev_half_viewport(),
        }
        Ok(Render::Yes)
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum LogsActions {
    Down,
    Up,
    DownHalf,
    UpHalf,
}
