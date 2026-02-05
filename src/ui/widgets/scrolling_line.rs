use std::time::Duration;

use bon::Builder;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, StyledGrapheme},
    widgets::{Paragraph, Widget},
};
use unicode_width::UnicodeWidthStr;

#[derive(Builder)]
pub struct ScrollingLine<'a> {
    scroll_speed: u64,
    align: Alignment,
    line: Line<'a>,
    progress: Duration,
}

impl Widget for ScrollingLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let line_len = self.line.width() as u64;
        let max_len = area.width as u64;

        if line_len <= max_len || self.scroll_speed == 0 {
            Paragraph::new(self.line).alignment(self.align).render(area, buf);
            return;
        }

        let mut x = 0u64;

        // +3 for the spaces and pipes
        let line_len = line_len + 3;
        let elapsed_ms = self.progress.as_millis() as u64;
        let cols_to_offset = ((elapsed_ms * self.scroll_speed) / 1000) % line_len;

        let mut acc = 0;

        let mut res = String::new();
        for StyledGrapheme { symbol, style } in self
            .line
            .styled_graphemes(Style::default())
            .chain(std::iter::once(StyledGrapheme::new(" ", Style::default())))
            .chain(std::iter::once(StyledGrapheme::new("|", Style::default())))
            .chain(std::iter::once(StyledGrapheme::new(" ", Style::default())))
            .chain(self.line.styled_graphemes(Style::default()))
            .skip_while(|StyledGrapheme { symbol, .. }| {
                let result = acc < cols_to_offset as usize;
                acc += symbol.width();
                result
            })
        {
            let width = symbol.width() as u64;
            if width == 0 {
                continue;
            }
            if x + width > max_len {
                break;
            }

            buf[(area.left() + x as u16, area.top())].set_symbol(symbol).set_style(style);
            res.push_str(symbol);
            x += width;
        }
    }
}
