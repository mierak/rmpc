use anyhow::Result;
use ratatui::Frame;
use ratatui::prelude::Rect;

use super::Pane;
use crate::context::AppContext;
use crate::mpd::mpd_client::{MpdClient, ValueChange};
use crate::shared::key_event::KeyEvent;
use crate::shared::mouse_event::{MouseEvent, MouseEventKind};
use crate::ui::UiEvent;
use crate::ui::widgets::header::Header;

#[derive(Debug)]
pub struct HeaderPane {
    area: Rect,
}

impl HeaderPane {
    pub fn new() -> Self {
        Self { area: Rect::default() }
    }
}

impl Pane for HeaderPane {
    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        context: &AppContext,
    ) -> anyhow::Result<()> {
        self.area = area;
        frame.render_widget(Header::new(context), self.area);
        Ok(())
    }

    fn before_show(&mut self, _context: &AppContext) -> Result<()> {
        Ok(())
    }

    fn on_event(
        &mut self,
        _event: &mut UiEvent,
        _is_visible: bool,
        _context: &AppContext,
    ) -> Result<()> {
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &AppContext) -> Result<()> {
        if !self.area.contains(event.into()) {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::LeftClick => {
                context.command(move |client| {
                    client.pause_toggle()?;
                    Ok(())
                });
            }
            MouseEventKind::ScrollUp => {
                context.command(|client| {
                    client.volume(ValueChange::Increase(context.config.volume_step.into()))?;
                    Ok(())
                });
            }
            MouseEventKind::ScrollDown => {
                context.command(|client| {
                    client.volume(ValueChange::Decrease(context.config.volume_step.into()))?;
                    Ok(())
                });
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _context: &mut AppContext) -> Result<()> {
        Ok(())
    }
}
