use anyhow::Result;
use itertools::Itertools;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    prelude::Rect,
    widgets::{List, ListState},
};

use super::Pane;
use crate::{
    config::keys::{CommonAction, LogsActions},
    ctx::Ctx,
    shared::{
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
        ring_vec::RingVec,
    },
    ui::{UiEvent, dirstack::DirState},
};

#[derive(Debug)]
pub struct LogsPane {
    logs: RingVec<1000, Vec<u8>>,
    scrolling_state: DirState<ListState>,
    logs_area: Rect,
    should_scroll_to_last: bool,
    scroll_enabled: bool,
}

impl LogsPane {
    pub fn new() -> Self {
        Self {
            scroll_enabled: true,
            logs: RingVec::default(),
            scrolling_state: DirState::default(),
            logs_area: Rect::default(),
            should_scroll_to_last: false,
        }
    }
}

const INDENT_LEN: usize = 4;
const INDENT: &str = "    ";

impl Pane for LogsPane {
    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        Ctx { config, .. }: &Ctx,
    ) -> anyhow::Result<()> {
        let scrollbar_area_width: u16 = config.theme.scrollbar.is_some().into();
        let [logs_area, scrollbar_area] = Layout::horizontal([
            Constraint::Percentage(100),
            Constraint::Min(scrollbar_area_width),
        ])
        .areas(area);
        self.logs_area = logs_area;

        let max_line_width = (logs_area.width as usize).saturating_sub(INDENT_LEN + 3);
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
        self.scrolling_state.set_content_and_viewport_len(content_len, logs_area.height.into());
        if self.scroll_enabled
            && (self.scrolling_state.get_selected().is_none() || self.should_scroll_to_last)
        {
            self.should_scroll_to_last = false;
            self.scrolling_state.last();
        }

        let logs_wg = List::new(lines)
            .style(config.as_text_style())
            .highlight_style(config.theme.current_item_style);
        if let Some(scrollbar) = config.as_styled_scrollbar() {
            frame.render_stateful_widget(
                scrollbar,
                scrollbar_area,
                self.scrolling_state.as_scrollbar_state_ref(),
            );
        }
        frame.render_stateful_widget(
            logs_wg,
            logs_area,
            self.scrolling_state.as_render_state_ref(),
        );

        Ok(())
    }

    fn before_show(&mut self, _ctx: &Ctx) -> Result<()> {
        self.scrolling_state.last();
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, is_visible: bool, ctx: &Ctx) -> Result<()> {
        if let UiEvent::LogAdded(msg) = event {
            self.logs.push(std::mem::take(msg));
            self.should_scroll_to_last = true;
            if is_visible {
                ctx.render()?;
            }
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        if !self.logs_area.contains(event.into()) {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::ScrollUp => {
                self.scrolling_state.scroll_up(1, ctx.config.scrolloff);

                ctx.render()?;
            }
            MouseEventKind::ScrollDown => {
                self.scrolling_state.scroll_down(1, ctx.config.scrolloff);

                ctx.render()?;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_action(&mut self, event: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        let config = &ctx.config;
        if let Some(action) = event.as_logs_action(ctx) {
            match action {
                LogsActions::Clear => {
                    self.logs.clear();

                    ctx.render()?;
                }
                LogsActions::ToggleScroll => {
                    self.scroll_enabled ^= true;
                }
            }
        } else if let Some(action) = event.as_common_action(ctx) {
            match action {
                CommonAction::DownHalf => {
                    self.scrolling_state.next_half_viewport(ctx.config.scrolloff);

                    ctx.render()?;
                }
                CommonAction::UpHalf => {
                    self.scrolling_state.prev_half_viewport(ctx.config.scrolloff);

                    ctx.render()?;
                }
                CommonAction::Up => {
                    self.scrolling_state.prev(ctx.config.scrolloff, config.wrap_navigation);

                    ctx.render()?;
                }
                CommonAction::Down => {
                    self.scrolling_state.next(ctx.config.scrolloff, config.wrap_navigation);

                    ctx.render()?;
                }
                CommonAction::Bottom => {
                    self.scrolling_state.last();

                    ctx.render()?;
                }
                CommonAction::Top => {
                    self.scrolling_state.first();

                    ctx.render()?;
                }
                _ => {}
            }
        }

        Ok(())
    }
}
