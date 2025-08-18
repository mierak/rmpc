use anyhow::Result;
use ratatui::{Frame, prelude::Rect};

use super::Pane;
use crate::{
    config::{tabs::VolumeType, theme::volume_slider::VolumeSliderConfig},
    ctx::Ctx,
    mpd::{
        commands::volume::Bound,
        mpd_client::{MpdClient, ValueChange},
    },
    shared::{
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::widgets::volume_slider::VolumeSlider,
};

#[derive(Debug)]
pub struct VolumePane {
    area: Rect,
    config: VolumeType,
}

impl VolumePane {
    pub fn new(config: VolumeType) -> Self {
        Self { area: Rect::default(), config }
    }
}

fn as_styled_volume_slider(config: &VolumeSliderConfig) -> VolumeSlider<'_> {
    VolumeSlider::default()
        .filled_style(config.filled_style)
        .thumb_style(config.thumb_style)
        .empty_style(config.track_style)
        .start_char(&config.symbols[0])
        .filled_char(&config.symbols[1])
        .thumb_char(&config.symbols[2])
        .empty_char(&config.symbols[3])
        .end_char(&config.symbols[4])
}

impl Pane for VolumePane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> anyhow::Result<()> {
        self.area = area;

        match &self.config {
            VolumeType::Slider(config) => {
                let volume_slider = as_styled_volume_slider(config)
                    .value(f64::from(*ctx.status.volume.value()) / 100.0);

                frame.render_widget(volume_slider, self.area);
            }
        }

        Ok(())
    }

    fn before_show(&mut self, _ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
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
                let new_volume = (volume_ratio * 100.0).clamp(0.0, 100.0).round() as u32;

                ctx.command(move |client| {
                    client.volume(ValueChange::Set(new_volume))?;
                    Ok(())
                });

                ctx.render()?;
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
