use std::collections::VecDeque;

use anyhow::Result;
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::{
    prelude::Rect,
    widgets::{Block, List, ListState, Padding},
    Frame,
};

use crate::{
    config::keys::{CommonAction, LogsActions},
    context::AppContext,
    mpd::mpd_client::MpdClient,
    ui::{utils::dirstack::DirState, KeyHandleResultInternal, UiEvent},
    utils::mouse_event::{MouseEvent, MouseEventKind},
};

use super::Pane;

#[derive(Debug, Default)]
pub struct LogsPane {
    logs: VecDeque<Vec<u8>>,
    scrolling_state: DirState<ListState>,
    logs_area: Rect,
}

impl Pane for LogsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, AppContext { config, .. }: &AppContext) -> anyhow::Result<()> {
        let lines: Vec<_> = self.logs.iter().map(|l| String::from_utf8_lossy(l)).collect_vec();

        let content_len = lines.len();
        self.scrolling_state.set_content_len(Some(content_len));
        self.scrolling_state.set_viewport_len(Some(area.height.into()));
        if self.scrolling_state.get_selected().is_none() {
            self.scrolling_state.last();
        }

        let logs_wg = List::new(lines)
            .style(config.as_text_style())
            .highlight_style(config.theme.current_item_style)
            .block(Block::default().padding(Padding::right(5)));
        frame.render_stateful_widget(logs_wg, area, self.scrolling_state.as_render_state_ref());
        frame.render_stateful_widget(
            config.as_styled_scrollbar(),
            area,
            self.scrolling_state.as_scrollbar_state_ref(),
        );
        self.logs_area = area;

        Ok(())
    }

    fn before_show(&mut self, _client: &mut impl MpdClient, _context: &AppContext) -> Result<()> {
        self.scrolling_state.last();
        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut UiEvent,
        _client: &mut impl MpdClient,
        _context: &AppContext,
    ) -> Result<KeyHandleResultInternal> {
        if let UiEvent::LogAdded(msg) = event {
            self.logs.push_back(std::mem::take(msg));
            if self.logs.len() > 1000 {
                self.logs.pop_front();
            }
            Ok(KeyHandleResultInternal::RenderRequested)
        } else {
            Ok(KeyHandleResultInternal::SkipRender)
        }
    }

    fn handle_mouse_event(
        &mut self,
        event: MouseEvent,
        _client: &mut impl MpdClient,
        context: &mut AppContext,
    ) -> Result<KeyHandleResultInternal> {
        if !self.logs_area.contains(event.into()) {
            return Ok(KeyHandleResultInternal::SkipRender);
        }

        match event.kind {
            MouseEventKind::ScrollUp => {
                self.scrolling_state.prev(context.config.scrolloff, false);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            MouseEventKind::ScrollDown => {
                self.scrolling_state.next(context.config.scrolloff, false);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }

    fn handle_action(
        &mut self,
        event: KeyEvent,
        _client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<KeyHandleResultInternal> {
        let config = context.config;
        if let Some(action) = config.keybinds.logs.get(&event.into()) {
            match action {
                LogsActions::Clear => {
                    self.logs.clear();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
            }
        } else if let Some(action) = config.keybinds.navigation.get(&event.into()) {
            match action {
                CommonAction::DownHalf => {
                    self.scrolling_state.next_half_viewport(context.config.scrolloff);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::UpHalf => {
                    self.scrolling_state.prev_half_viewport(context.config.scrolloff);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Up => {
                    self.scrolling_state
                        .prev(context.config.scrolloff, config.wrap_navigation);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    self.scrolling_state
                        .next(context.config.scrolloff, config.wrap_navigation);
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
                CommonAction::Close => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Confirm => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::FocusInput => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::AddAll => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::PaneDown => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::PaneUp => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::PaneRight => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::PaneLeft => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}
