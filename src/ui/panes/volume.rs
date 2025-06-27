use anyhow::Result;
use ratatui::{Frame, prelude::Rect};

use super::Pane;
use crate::{
    context::AppContext,
    mpd::{
        commands::volume::Bound,
        mpd_client::{MpdClient, ValueChange},
    },
    shared::{
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
};

#[derive(Debug)]
pub struct VolumePane {
    area: Rect,
}

impl VolumePane {
    pub fn new() -> Self {
        Self { area: Rect::default() }
    }
}

impl Pane for VolumePane {
    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        context: &AppContext,
    ) -> anyhow::Result<()> {
        self.area = area;

        let volume_slider = context
            .config
            .as_styled_volume_slider()
            .value(f32::from(*context.status.volume.value()) / 100.0);

        frame.render_widget(volume_slider, self.area);

        Ok(())
    }

    fn before_show(&mut self, _context: &AppContext) -> Result<()> {
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &AppContext) -> Result<()> {
        if !self.area.contains(event.into()) {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick => {
                // Avoid division by zero (if width is set to 0)
                if self.area.width == 0 {
                    return Ok(());
                }

                let volume_ratio =
                    f32::from(event.x.saturating_sub(self.area.x)) / f32::from(self.area.width);

                // Safe conversion: clamped to 0-100 range and rounded, so cast is always valid
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let new_volume = (volume_ratio * 100.0).clamp(0.0, 100.0).round() as u32;

                context.command(move |client| {
                    client.volume(ValueChange::Set(new_volume))?;
                    Ok(())
                });

                context.render()?;
            }
            MouseEventKind::ScrollUp => {
                let volume_step = context.config.volume_step.into();
                context.command(move |client| {
                    client.volume(ValueChange::Increase(volume_step))?;
                    Ok(())
                });
            }
            MouseEventKind::ScrollDown => {
                let volume_step = context.config.volume_step.into();
                context.command(move |client| {
                    client.volume(ValueChange::Decrease(volume_step))?;
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
