use anyhow::Result;
use ratatui::{Frame, prelude::Rect, style::Stylize, text::Text};

use super::Pane;
use crate::{ctx::Ctx, shared::key_event::KeyEvent};

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
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &Ctx) -> anyhow::Result<()> {
        self.area = area;
        let text = format!("{} frames", context.rendered_frames);
        frame.render_widget(
            Text::from(text).fg(context.config.theme.text_color.unwrap_or_default()).bg(context
                .config
                .theme
                .background_color
                .unwrap_or_default()),
            area,
        );

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _context: &mut Ctx) -> Result<()> {
        Ok(())
    }
}
