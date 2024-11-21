use std::collections::VecDeque;

use anyhow::Result;
use itertools::Itertools;
use ratatui::{
    prelude::Rect,
    widgets::{List, ListState},
    Frame,
};

use crate::{
    config::keys::{CommonAction, LogsActions},
    context::AppContext,
    mpd::mpd_client::MpdClient,
    shared::{
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::{dirstack::DirState, UiEvent},
};

use super::Pane;

#[derive(Debug)]
pub struct LogsPane {
    logs: VecDeque<Vec<u8>>,
    scrolling_state: DirState<ListState>,
    logs_area: Rect,
    should_scroll_to_last: bool,
    scroll_enabled: bool,
}

impl LogsPane {
    pub fn new() -> Self {
        Self {
            scroll_enabled: true,
            logs: VecDeque::new(),
            scrolling_state: DirState::default(),
            logs_area: Rect::default(),
            should_scroll_to_last: false,
        }
    }
}

const INDENT_LEN: usize = 4;
const INDENT: &str = "    ";

impl Pane for LogsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, AppContext { config, .. }: &AppContext) -> anyhow::Result<()> {
        let max_line_width = (area.width as usize).saturating_sub(INDENT_LEN + 3);
        let lines: Vec<_> = self.logs.iter().map(|l| String::from_utf8_lossy(l)).collect_vec();
        let lines: Vec<_> = lines
            .iter()
            .flat_map(|l| {
                let mut lines = textwrap::wrap(l, textwrap::Options::new(max_line_width));
                lines
                    .iter_mut()
                    .skip(1)
                    .for_each(|v| *v = std::borrow::Cow::Owned(textwrap::indent(v, INDENT)));
                lines
            })
            .collect();

        let content_len = lines.len();
        self.scrolling_state.set_content_len(Some(content_len));
        self.scrolling_state.set_viewport_len(Some(area.height.into()));
        if self.scroll_enabled && (self.scrolling_state.get_selected().is_none() || self.should_scroll_to_last) {
            self.should_scroll_to_last = false;
            self.scrolling_state.last();
        }

        let logs_wg = List::new(lines)
            .style(config.as_text_style())
            .highlight_style(config.theme.current_item_style);
        frame.render_stateful_widget(
            config.as_styled_scrollbar(),
            area,
            self.scrolling_state.as_scrollbar_state_ref(),
        );
        let mut area = area;
        area.width = area.width.saturating_sub(1);
        frame.render_stateful_widget(logs_wg, area, self.scrolling_state.as_render_state_ref());
        self.logs_area = area;

        Ok(())
    }

    fn before_show(&mut self, _client: &mut impl MpdClient, _context: &AppContext) -> Result<()> {
        self.scrolling_state.last();
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, _client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        if let UiEvent::LogAdded(msg) = event {
            self.logs.push_back(std::mem::take(msg));
            if self.logs.len() > 1000 {
                self.logs.pop_front();
            }
            self.should_scroll_to_last = true;
            context.render()?;
        }

        Ok(())
    }

    fn handle_mouse_event(
        &mut self,
        event: MouseEvent,
        _client: &mut impl MpdClient,
        context: &mut AppContext,
    ) -> Result<()> {
        if !self.logs_area.contains(event.into()) {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::ScrollUp => {
                self.scrolling_state.prev(context.config.scrolloff, false);

                context.render()?;
            }
            MouseEventKind::ScrollDown => {
                self.scrolling_state.next(context.config.scrolloff, false);

                context.render()?;
            }
            _ => {}
        };

        Ok(())
    }

    fn handle_action(
        &mut self,
        event: &mut KeyEvent,
        _client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<()> {
        let config = context.config;
        if let Some(action) = event.as_logs_action(context) {
            match action {
                LogsActions::Clear => {
                    self.logs.clear();

                    context.render()?;
                }
                LogsActions::ToggleScroll => {
                    self.scroll_enabled ^= true;
                }
            }
        } else if let Some(action) = event.as_common_action(context) {
            match action {
                CommonAction::DownHalf => {
                    self.scrolling_state.next_half_viewport(context.config.scrolloff);

                    context.render()?;
                }
                CommonAction::UpHalf => {
                    self.scrolling_state.prev_half_viewport(context.config.scrolloff);

                    context.render()?;
                }
                CommonAction::Up => {
                    self.scrolling_state
                        .prev(context.config.scrolloff, config.wrap_navigation);

                    context.render()?;
                }
                CommonAction::Down => {
                    self.scrolling_state
                        .next(context.config.scrolloff, config.wrap_navigation);

                    context.render()?;
                }
                CommonAction::Bottom => {
                    self.scrolling_state.last();

                    context.render()?;
                }
                CommonAction::Top => {
                    self.scrolling_state.first();

                    context.render()?;
                }
                CommonAction::Right => {}
                CommonAction::Left => {}
                CommonAction::EnterSearch => {}
                CommonAction::NextResult => {}
                CommonAction::PreviousResult => {}
                CommonAction::Add => {}
                CommonAction::Select => {}
                CommonAction::InvertSelection => {}
                CommonAction::Delete => {}
                CommonAction::Rename => {}
                CommonAction::MoveUp => {}
                CommonAction::MoveDown => {}
                CommonAction::Close => {}
                CommonAction::Confirm => {}
                CommonAction::FocusInput => {}
                CommonAction::AddAll => {}
                CommonAction::PaneDown => {}
                CommonAction::PaneUp => {}
                CommonAction::PaneRight => {}
                CommonAction::PaneLeft => {}
            }
        }

        Ok(())
    }
}
