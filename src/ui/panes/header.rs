use anyhow::Result;
use ratatui::{Frame, prelude::Rect};

use super::Pane;
use crate::{
    ctx::Ctx,
    mpd::mpd_client::{MpdClient, ValueChange},
    shared::{
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::{UiEvent, widgets::header::Header},
};

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
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> anyhow::Result<()> {
        self.area = area;
        frame.render_widget(Header::new(ctx), self.area);
        Ok(())
    }

    fn before_show(&mut self, _ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn on_event(&mut self, _event: &mut UiEvent, _is_visible: bool, _ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        if !self.area.contains(event.into()) {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::LeftClick => {
                ctx.command(move |client| {
                    client.pause_toggle()?;
                    Ok(())
                });
            }
            MouseEventKind::ScrollUp => {
                let volume_step = ctx.config.volume_step.into();
                ctx.command(move |client| {
                    client.volume(ValueChange::Increase(volume_step))?;
                    Ok(())
                });
            }
            MouseEventKind::ScrollDown => {
                let volume_step = ctx.config.volume_step.into();
                ctx.command(move |client| {
                    client.volume(ValueChange::Decrease(volume_step))?;
                    Ok(())
                });
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}
