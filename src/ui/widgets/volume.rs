use ratatui::prelude::Alignment;
use ratatui::style::Style;
use ratatui::widgets::{Block, Widget};

use super::get_line_offset;

const CHARS: &[&str] = &["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

#[derive(Debug)]
pub struct Volume<'a> {
    value: u8,
    block: Option<Block<'a>>,
    alignment: Alignment,
    style: Style,
}

impl Default for Volume<'_> {
    fn default() -> Self {
        Self { value: 0, block: None, alignment: Alignment::Left, style: Style::default() }
    }
}

#[allow(dead_code)]
impl<'a> Volume<'a> {
    pub fn value(mut self, value: u8) -> Self {
        self.value = value;
        self
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
}

impl Volume<'_> {
    pub fn get_str(value: u8) -> String {
        let i = std::cmp::min((value / 13) as usize, CHARS.len());
        format!("Volume: {:<7} {:>3}%", CHARS[0..i].join(""), value)
    }
}

impl Widget for Volume<'_> {
    fn render(mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
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

        let left_offset = get_line_offset(20, area.width, self.alignment);

        let i = self.value / 13;
        buf.set_string(
            area.left() + left_offset,
            area.top(),
            format!("Volume: {:<7} {:>3}%", CHARS[0..i as usize].join(""), self.value),
            self.style,
        );
    }
}
