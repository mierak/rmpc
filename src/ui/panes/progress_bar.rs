use std::time::Duration;

use anyhow::Result;
use ratatui::{Frame, prelude::Rect, widgets::Paragraph};

use super::Pane;
use crate::{
    context::Ctx,
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
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &Ctx) -> anyhow::Result<()> {
        self.area = area;

        match context.messages.last() {
            Some(status) if status.created.elapsed() < status.timeout => {
                let status_bar = Paragraph::new(status.message.clone())
                    .alignment(ratatui::prelude::Alignment::Center)
                    .style(status.level.into_style(&context.config.theme.level_styles));
                frame.render_widget(status_bar, self.area);
            }
            _ => {
                let elapsed_bar = context.config.as_styled_progress_bar();
                let elapsed_bar = if context.status.duration == Duration::ZERO {
                    elapsed_bar.value(0.0)
                } else {
                    elapsed_bar.value(
                        context.status.elapsed.as_secs_f32()
                            / context.status.duration.as_secs_f32(),
                    )
                };
                frame.render_widget(elapsed_bar, self.area);
            }
        }

        Ok(())
    }

    fn before_show(&mut self, _context: &Ctx) -> Result<()> {
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &Ctx) -> Result<()> {
        if !self.area.contains(event.into()) {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick
                if matches!(context.status.state, State::Play | State::Pause) =>
            {
                let second_to_seek_to = context
                    .status
                    .duration
                    .mul_f32(
                        f32::from(event.x.saturating_sub(self.area.x)) / f32::from(self.area.width),
                    )
                    .as_secs();
                context.command(move |client| {
                    client.seek_current(ValueChange::Set(u32::try_from(second_to_seek_to)?))?;
                    Ok(())
                });

                context.render()?;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _context: &mut Ctx) -> Result<()> {
        Ok(())
    }
}
