use ratatui::{
    prelude::Alignment,
    style::Style,
    widgets::{Block, Widget},
};
use std::time::Instant;

use super::get_line_offset;

const DEFAULT_LOADING_CHARS: [&str; 8] = ["⣻", "⣽", "⣾", "⣷", "⣯", "⣟", "⡿", "⢿"];

#[derive(Debug)]
pub struct ScanStatus<'a> {
    block: Option<Block<'a>>,
    alignment: Alignment,
    style: Style,
    update_start: Option<Instant>,
}

#[allow(dead_code)]
impl<'a> ScanStatus<'a> {
    pub fn new(update_start: Option<Instant>) -> Self {
        Self { block: None, alignment: Alignment::Left, style: Style::default(), update_start }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// get updating symbol, this symbol rotates in set inverval if the db is
    /// scanning
    pub fn get_str(&mut self) -> String {
        let Some(start) = self.update_start else {
            return String::new();
        };
        let elapsed_secs = start.elapsed().as_millis() as usize / 1000;
        let t =
            DEFAULT_LOADING_CHARS.get(elapsed_secs % DEFAULT_LOADING_CHARS.len()).unwrap_or(&"");
        format!(" {t} ")
    }
}

impl Widget for &mut ScanStatus<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        if area.height < 1 {
            return;
        }

        let left_offset = get_line_offset(3, area.width, self.alignment);

        buf.set_string(area.left() + left_offset, area.top(), self.get_str(), self.style);
    }
}
