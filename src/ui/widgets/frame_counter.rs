use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Widget;
use std::time::Instant;

#[derive(Debug)]
pub struct FrameCounter {
    pub frame_count: u64,
    pub start_time: Instant,
}
impl Default for FrameCounter {
    fn default() -> Self {
        Self {
            frame_count: 0,
            start_time: Instant::now(),
        }
    }
}

impl FrameCounter {
    pub fn fps(&self) -> f64 {
        self.frame_count as f64
    }

    pub fn increment(&mut self) {
        self.frame_count += 1;
    }

    pub fn reset(&mut self) {
        self.frame_count = 0;
        self.start_time = Instant::now();
    }
}

impl Widget for &FrameCounter {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let text = format!("Rendered frames: {}", self.fps());
        buf.set_string(area.left(), area.top(), text, Style::default());
    }
}
