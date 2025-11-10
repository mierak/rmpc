use anyhow::Result;
use ratatui::{Frame, prelude::Rect};

use super::Pane;
use crate::{
    config::tabs::VolumeType,
    ctx::Ctx,
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
    config: VolumeType,
}

impl VolumePane {
    pub fn new(config: VolumeType) -> Self {
        Self { area: Rect::default(), config }
    }
}

impl Pane for VolumePane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> anyhow::Result<()> {
        self.area = area;

        match &self.config {
            VolumeType::Slider(config) => {
                if area.height < 1 || area.width < 1 {
                    return Ok(());
                }

                let symbols = &config.symbols;
                let filled_len = (f64::from(area.width - 1) * f64::from(*ctx.status.volume.value())
                    / 100.0) as u16;

                for i in 0..area.width {
                    let style = if i <= filled_len && filled_len > 0 {
                        config.filled_style
                    } else {
                        config.track_style
                    };

                    let (c, style) = if let Some(sym) = &config.symbols.start
                        && i == 0
                    {
                        (sym, style)
                    } else if i < filled_len {
                        (&symbols.filled, style)
                    } else if let Some(sym) = &config.symbols.end
                        && i == area.width - 1
                    {
                        (sym, style)
                    } else if i == filled_len {
                        (&symbols.thumb, config.thumb_style)
                    } else {
                        (&symbols.track, style)
                    };

                    frame.buffer_mut().set_string(area.x + i, area.y, c, style);
                }
            }
        }

        Ok(())
    }

    fn before_show(&mut self, _ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick => {
                if !self.area.contains(event.into()) {
                    return Ok(());
                }
                // Avoid division by zero (if width is set to 0)
                if self.area.width == 0 {
                    return Ok(());
                }

                let volume_ratio =
                    f32::from(event.x.saturating_sub(self.area.x)) / f32::from(self.area.width - 1);

                // Safe conversion: clamped to 0-100 range and rounded, so cast is always valid
                let new_volume = (volume_ratio * 100.0).clamp(0.0, 100.0).round() as u32;

                ctx.command(move |client| {
                    client.volume(ValueChange::Set(new_volume))?;
                    Ok(())
                });

                ctx.render()?;
            }
            MouseEventKind::ScrollUp => {
                if !self.area.contains(event.into()) {
                    return Ok(());
                }
                let volume_step = ctx.config.volume_step.into();
                ctx.command(move |client| {
                    client.volume(ValueChange::Increase(volume_step))?;
                    Ok(())
                });
            }
            MouseEventKind::ScrollDown => {
                if !self.area.contains(event.into()) {
                    return Ok(());
                }
                let volume_step = ctx.config.volume_step.into();
                ctx.command(move |client| {
                    client.volume(ValueChange::Decrease(volume_step))?;
                    Ok(())
                });
            }
            MouseEventKind::Drag { drag_start_position } => {
                if !self.area.contains(drag_start_position) {
                    return Ok(());
                }

                let volume_ratio =
                    f32::from(event.x.saturating_sub(self.area.x)) / f32::from(self.area.width - 1);

                // Safe conversion: clamped to 0-100 range and rounded, so cast is always valid
                let new_volume = (volume_ratio * 100.0).clamp(0.0, 100.0).round() as u32;

                ctx.command(move |client| {
                    client.volume(ValueChange::Set(new_volume))?;
                    Ok(())
                });

                ctx.render()?;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend, prelude::Rect};
    use rstest::rstest;

    use crate::{
        config::{
            tabs::VolumeType,
            theme::volume_slider::{Symbols, VolumeSliderConfig},
        },
        ctx::Ctx,
        mpd::commands::Volume,
        tests::fixtures::{ctx, terminal},
        ui::panes::{Pane, volume::VolumePane},
    };

    fn pane() -> VolumePane {
        VolumePane::new(VolumeType::Slider(VolumeSliderConfig {
            symbols: Symbols {
                start: Some("♪".to_owned()),
                filled: "█".to_owned(),
                thumb: "●".to_owned(),
                track: "─".to_owned(),
                end: Some("♪".to_owned()),
            },
            ..Default::default()
        }))
    }

    #[rstest]
    fn volume_zero_is_correct(mut terminal: Terminal<TestBackend>, mut ctx: Ctx) {
        let mut pane = pane();
        ctx.status.volume = Volume::new(0);

        let buf = terminal
            .draw(|frame| {
                pane.render(frame, Rect::new(0, 0, 5, 1), &ctx).unwrap();
            })
            .unwrap()
            .buffer;

        assert_eq!(buf[(0, 0)].symbol(), "♪");
        assert_eq!(buf[(1, 0)].symbol(), "─");
        assert_eq!(buf[(2, 0)].symbol(), "─");
        assert_eq!(buf[(3, 0)].symbol(), "─");
        assert_eq!(buf[(4, 0)].symbol(), "♪");
    }

    #[rstest]
    fn volume_max_is_correct(mut terminal: Terminal<TestBackend>, mut ctx: Ctx) {
        let mut pane = pane();
        ctx.status.volume = Volume::new(100);

        let buf = terminal
            .draw(|frame| {
                pane.render(frame, Rect::new(0, 0, 5, 1), &ctx).unwrap();
            })
            .unwrap()
            .buffer;

        assert_eq!(buf[(0, 0)].symbol(), "♪");
        assert_eq!(buf[(1, 0)].symbol(), "█");
        assert_eq!(buf[(2, 0)].symbol(), "█");
        assert_eq!(buf[(3, 0)].symbol(), "█");
        assert_eq!(buf[(4, 0)].symbol(), "♪");
    }

    #[rstest]
    fn volume_half_is_correct(mut terminal: Terminal<TestBackend>, mut ctx: Ctx) {
        let mut pane = pane();
        ctx.status.volume = Volume::new(50);

        let buf = terminal
            .draw(|frame| {
                pane.render(frame, Rect::new(0, 0, 5, 1), &ctx).unwrap();
            })
            .unwrap()
            .buffer;

        assert_eq!(buf[(0, 0)].symbol(), "♪");
        assert_eq!(buf[(1, 0)].symbol(), "█");
        assert_eq!(buf[(2, 0)].symbol(), "●");
        assert_eq!(buf[(3, 0)].symbol(), "─");
        assert_eq!(buf[(4, 0)].symbol(), "♪");
    }
}
