use std::time::Duration;

use anyhow::Result;
use ratatui::{
    prelude::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

use crate::{
    context::AppContext,
    mpd::mpd_client::{MpdClient, ValueChange},
    shared::{
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::{StatusMessage, UiEvent},
};

use super::Pane;

#[derive(Debug)]
pub struct ProgressBarPane {
    area: Rect,
    status_message: Option<StatusMessage>,
}

impl ProgressBarPane {
    pub fn new() -> Self {
        Self {
            area: Rect::default(),

            status_message: None,
        }
    }
}

impl Pane for ProgressBarPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> anyhow::Result<()> {
        self.area = area;
        if self
            .status_message
            .as_ref()
            .is_some_and(|m| m.created.elapsed() > std::time::Duration::from_secs(5))
        {
            self.status_message = None;
        }

        if let Some(StatusMessage { message, level, .. }) = &self.status_message {
            let status_bar = Paragraph::new(message.to_owned())
                .alignment(ratatui::prelude::Alignment::Center)
                .style(Style::default().fg(level.into()).bg(Color::Black));
            frame.render_widget(status_bar, self.area);
        } else {
            let elapsed_bar = context.config.as_styled_progress_bar();
            let elapsed_bar = if context.status.duration == Duration::ZERO {
                elapsed_bar.value(0.0)
            } else {
                elapsed_bar.value(context.status.elapsed.as_secs_f32() / context.status.duration.as_secs_f32())
            };
            frame.render_widget(elapsed_bar, self.area);
        }
        Ok(())
    }

    fn before_show(&mut self, _context: &AppContext) -> Result<()> {
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, _is_visible: bool, _context: &AppContext) -> Result<()> {
        match event {
            UiEvent::Status(message, level) => {
                self.status_message = Some(StatusMessage {
                    message: std::mem::take(message),
                    level: *level,
                    created: std::time::Instant::now(),
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &AppContext) -> Result<()> {
        if !self.area.contains(event.into()) {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick => {
                let second_to_seek_to = context
                    .status
                    .duration
                    .mul_f32(f32::from(event.x.saturating_sub(self.area.x)) / f32::from(self.area.width))
                    .as_secs();
                context.command(move |client| {
                    client.seek_current(ValueChange::Set(u32::try_from(second_to_seek_to)?))?;
                    Ok(())
                });

                context.render()?;
            }
            _ => {}
        };

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _context: &mut AppContext) -> Result<()> {
        Ok(())
    }
}
