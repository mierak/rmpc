use std::time::Duration;

use anyhow::Result;
use ratatui::{Frame, prelude::Rect, widgets::Paragraph};

use super::Pane;
use crate::{
    ctx::Ctx,
    mpd::{
        commands::State,
        mpd_client::{MpdClient, ValueChange},
    },
    shared::{
        keys::ActionEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::widgets::progress_bar::ProgressBar,
};

#[derive(Debug)]
pub struct ProgressBarPane {
    area: Rect,
}

impl ProgressBarPane {
    pub fn new() -> Self {
        Self { area: Rect::default() }
    }
}

impl Pane for ProgressBarPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> anyhow::Result<()> {
        self.area = area;

        match ctx.messages.last() {
            Some(status) if status.created.elapsed() < status.timeout => {
                let status_bar = Paragraph::new(status.message.clone())
                    .alignment(ratatui::prelude::Alignment::Center)
                    .style(status.level.into_style(&ctx.config.theme.level_styles));
                frame.render_widget(status_bar, self.area);
            }
            _ => {
                let bar_cfg = &ctx.config.theme.progress_bar;
                let value = if ctx.status.duration == Duration::ZERO {
                    0.0
                } else {
                    ctx.status.elapsed.as_secs_f32() / ctx.status.duration.as_secs_f32()
                };
                let bar = ProgressBar::builder()
                    .elapsed_style(bar_cfg.elapsed_style)
                    .thumb_style(bar_cfg.thumb_style)
                    .track_style(bar_cfg.track_style)
                    .start_char(&bar_cfg.symbols[0])
                    .elapsed_char(&bar_cfg.symbols[1])
                    .thumb_char(&bar_cfg.symbols[2])
                    .track_char(&bar_cfg.symbols[3])
                    .end_char(&bar_cfg.symbols[4])
                    .use_track_when_empty(ctx.config.theme.progress_bar.use_track_when_empty)
                    .value(value)
                    .build();

                frame.render_widget(bar, self.area);
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
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick
                if matches!(ctx.status.state, State::Play | State::Pause) =>
            {
                let second_to_seek_to = ctx
                    .status
                    .duration
                    .mul_f32(
                        f32::from(event.x.saturating_sub(self.area.x)) / f32::from(self.area.width),
                    )
                    .as_secs();
                ctx.command(move |client| {
                    client.seek_current(ValueChange::Set(u32::try_from(second_to_seek_to)?))?;
                    Ok(())
                });

                ctx.render()?;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut ActionEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}
