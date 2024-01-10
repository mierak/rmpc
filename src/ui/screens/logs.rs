use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::{
    prelude::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Span,
    widgets::{Block, Borders, List, ListItem, ListState, Scrollbar, ScrollbarOrientation},
    Frame,
};
use strum::Display;

use crate::{
    mpd::client::Client,
    state::State,
    ui::{utils::dirstack::DirState, KeyHandleResultInternal, SharedUiState},
};

use super::{CommonAction, Screen};

#[derive(Debug, Default)]
pub struct LogsScreen {
    scrolling_state: DirState<ListState>,
}

impl Screen for LogsScreen {
    type Actions = LogsActions;
    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> anyhow::Result<()> {
        let lines: Vec<_> = app
            .logs
            .iter()
            .map(|l| -> Result<_> { Ok(l.into_text()?.lines) })
            .flatten_ok()
            .enumerate()
            .map(|(idx, l)| -> Result<_> {
                match l {
                    Ok(mut val) => {
                        if self.scrolling_state.get_selected().is_some_and(|v| v == idx) {
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
            .constraints([Constraint::Percentage(100), Constraint::Min(0)].as_ref())
            .split(area)
        else {
            return Ok(());
        };

        let content_len = lines.len();
        self.scrolling_state.set_content_len(Some(content_len));
        self.scrolling_state.set_viewport_len(Some(content.height.into()));

        let logs_wg = List::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Gray))
                .title(Span::styled(
                    format!("Logs: {content_len}"),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
        );

        frame.render_stateful_widget(logs_wg, content, self.scrolling_state.as_render_state_ref());
        frame.render_stateful_widget(
            scrollbar,
            scroll.inner(&Margin {
                vertical: 1,
                horizontal: 0,
            }),
            self.scrolling_state.as_scrollbar_state_ref(),
        );

        Ok(())
    }

    fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        self.scrolling_state.last();
        Ok(())
    }

    fn handle_action(
        &mut self,
        event: KeyEvent,
        _client: &mut Client<'_>,
        app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<KeyHandleResultInternal> {
        if let Some(action) = app.config.keybinds.logs.get(&event.into()) {
            match action {
                LogsActions::Clear => {
                    app.logs.clear();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
            }
        } else if let Some(action) = app.config.keybinds.navigation.get(&event.into()) {
            match action {
                CommonAction::DownHalf => {
                    self.scrolling_state.next_half_viewport();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::UpHalf => {
                    self.scrolling_state.prev_half_viewport();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Up => {
                    self.scrolling_state.prev();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    self.scrolling_state.next();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Bottom => {
                    self.scrolling_state.last();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Top => {
                    self.scrolling_state.first();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Right => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Left => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::EnterSearch => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::NextResult => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::PreviousResult => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Add => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Select => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Delete => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Rename => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::MoveUp => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::MoveDown => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum LogsActions {
    Clear,
}
