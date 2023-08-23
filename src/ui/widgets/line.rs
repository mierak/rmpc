use ratatui::{
    prelude::Alignment,
    style::Style,
    widgets::{Block, Widget},
};

use super::get_line_offset;

#[derive(Debug)]
pub struct Line<'a> {
    pub words: Vec<(String, Style)>,
    alignment: Alignment,
    block: Option<Block<'a>>,
    separator: String,
    separator_style: Style,
}
impl<'a> Default for Line<'a> {
    fn default() -> Self {
        Self {
            words: Vec::new(),
            alignment: Alignment::Left,
            block: None,
            separator: "".to_owned(),
            separator_style: Style::default(),
        }
    }
}

#[allow(dead_code)]
impl<'a> Line<'a> {
    pub fn new(words: Vec<(String, Style)>) -> Self {
        Self {
            words,
            alignment: Alignment::Left,
            block: None,
            separator: "".to_owned(),
            separator_style: Style::default(),
        }
    }

    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn separator(mut self, separator: String) -> Self {
        self.separator = separator;
        self
    }

    pub fn separator_style(mut self, separator_style: Style) -> Self {
        self.separator_style = separator_style;
        self
    }
}

impl<'a> Widget for Line<'a> {
    fn render(mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };
        let separator_len = self.separator.chars().count();

        let content_len = self.words.iter().fold(0, |mut acc, (w, _)| {
            acc += w.chars().count() + separator_len;
            acc
        }) - separator_len;

        let width = area.width;
        let top = area.top();
        let left_offset = get_line_offset(content_len as u16, area.width, self.alignment);

        let mut chars_written = 0;
        for (word, style) in
            itertools::Itertools::intersperse(self.words.iter(), &(self.separator, self.separator_style))
        {
            let len = word.chars().count() as u16;
            let x = area.left() + left_offset + chars_written;
            let chars_left = width.saturating_sub(chars_written);
            buf.set_stringn(x, top, word, chars_left as usize, *style);
            chars_written += len;
        }
    }
}
