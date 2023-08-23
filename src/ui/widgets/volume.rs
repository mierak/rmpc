use ratatui::{
    style::Style,
    widgets::{Block, Widget},
};

const CHARS: &[&str] = &["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

#[derive(Default, Debug)]
pub struct Volume<'a> {
    value: u8,
    block: Option<Block<'a>>,
    /// Widget style
    style: Style,
}

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
        let i = self.value / 13;
        buf.set_string(
            area.left(),
            area.top(),
            format!(" {} {}%", CHARS[0..i as usize].join(""), self.value),
            self.style,
        )
    }
}
