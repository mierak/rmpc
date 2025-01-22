use anyhow::Result;
use ratatui::{prelude::Rect, style::Stylize, text::Text, Frame};

use crate::{context::AppContext, shared::key_event::KeyEvent};

use super::Pane;

#[derive(Debug)]
pub struct FrameCountPane {
    area: Rect,
}

impl FrameCountPane {
    pub fn new() -> Self {
        Self { area: Rect::default() }
    }
}

impl Pane for FrameCountPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> anyhow::Result<()> {
        self.area = area;
        let text = format!("{} frames", context.rendered_frames);
        frame.render_widget(
            Text::from(text)
                .fg(context.config.theme.text_color.unwrap_or_default())
                .bg(context.config.theme.background_color.unwrap_or_default()),
            area,
        );

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _context: &mut AppContext) -> Result<()> {
        Ok(())
    }
}
