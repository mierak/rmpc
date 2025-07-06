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
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
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
                let elapsed_bar = ctx.config.as_styled_progress_bar();
                let elapsed_bar = if ctx.status.duration == Duration::ZERO {
                    elapsed_bar.value(0.0)
                } else {
                    elapsed_bar
                        .value(ctx.status.elapsed.as_secs_f32() / ctx.status.duration.as_secs_f32())
                };
                frame.render_widget(elapsed_bar, self.area);
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

    fn handle_action(&mut self, _event: &mut KeyEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}
