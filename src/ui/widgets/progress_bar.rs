use ratatui::{
    prelude::{Buffer, Rect},
    style::{Color, Style},
    widgets::Widget,
};

#[derive(Clone)]
pub struct ProgressBar<'a> {
    value: f32,
    elapsed_char: &'a str,
    track_char: &'a str,
    thumb_char: &'a str,
    elapsed_style: Style,
    track_style: Style,
    thumb_style: Style,
}

#[allow(dead_code)]
impl<'a> ProgressBar<'a> {
    pub fn value(mut self, val: f32) -> Self {
        self.value = val;
        self
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.elapsed_style = self.elapsed_style.fg(color);
        self.thumb_style = self.thumb_style.fg(color);
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.track_style = self.track_style.fg(color);
        self.thumb_style = self.thumb_style.bg(color);
        self
    }

    pub fn track_char(mut self, track: &'a str) -> Self {
        self.track_char = track;
        self
    }

    pub fn thumb_char(mut self, thumb: &'a str) -> Self {
        self.thumb_char = thumb;
        self
    }

    pub fn elapsed_char(mut self, elapsed: &'a str) -> Self {
        self.elapsed_char = elapsed;
        self
    }

    pub fn elapsed_style(mut self, style: Style) -> Self {
        self.elapsed_style = style;
        self
    }

    pub fn track_style(mut self, style: Style) -> Self {
        self.track_style = style;
        self
    }

    pub fn thumb_style(mut self, style: Style) -> Self {
        self.thumb_style = style;
        self
    }
}

impl Widget for ProgressBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let left = area.left();
        let right = area.right();
        let len = right - left;
        buf.set_string(
            area.left(),
            area.top(),
            self.track_char.repeat(len as usize),
            self.track_style,
        );

        let elapsed_len = (len as f32 * self.value) as usize;
        buf.set_string(
            area.left(),
            area.top(),
            self.elapsed_char.repeat(elapsed_len),
            self.elapsed_style,
        );
        if elapsed_len < len as usize && elapsed_len > 0 {
            buf.set_string(
                area.left() + elapsed_len as u16,
                area.top(),
                self.thumb_char,
                self.thumb_style,
            );
        }
    }
}

impl Default for ProgressBar<'_> {
    fn default() -> Self {
        Self {
            value: 0.0,
            elapsed_char: "█",
            track_char: " ",
            thumb_char: "",
            elapsed_style: Style::default().fg(Color::Blue),
            track_style: Style::default().bg(Color::Black),
            thumb_style: Style::default().bg(Color::Black).fg(Color::Blue),
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{
        buffer::Cell,
        prelude::{Buffer, Rect},
        widgets::Widget,
    };

    use super::ProgressBar;

    #[test]
    fn lower_bound_is_correct() {
        let wg = ProgressBar {
            thumb_char: "T",
            track_char: "B",
            elapsed_char: "E",
            ..Default::default()
        }
        .value(0.0);
        let area = Rect::new(0, 0, 3, 1);
        let mut buf = Buffer { area, content: vec![Cell::default(); 3] };

        wg.render(area, &mut buf);

        assert_eq!(buf[(0, 0)].symbol(), "B");
        assert_eq!(buf[(1, 0)].symbol(), "B");
        assert_eq!(buf[(2, 0)].symbol(), "B");
    }

    #[test]
    fn upper_bound_is_correct() {
        let wg = ProgressBar {
            thumb_char: "T",
            track_char: "B",
            elapsed_char: "E",
            ..Default::default()
        }
        .value(1.0);
        let area = Rect::new(0, 0, 3, 1);
        let mut buf = Buffer { area, content: vec![Cell::default(); 3] };

        wg.render(area, &mut buf);

        assert_eq!(buf[(0, 0)].symbol(), "E");
        assert_eq!(buf[(1, 0)].symbol(), "E");
        assert_eq!(buf[(2, 0)].symbol(), "E");
    }

    #[test]
    fn middle_is_correct() {
        let wg = ProgressBar {
            thumb_char: "T",
            track_char: "B",
            elapsed_char: "E",
            ..Default::default()
        }
        .value(0.5);
        let area = Rect::new(0, 0, 3, 1);
        let mut buf = Buffer { area, content: vec![Cell::default(); 3] };

        wg.render(area, &mut buf);

        assert_eq!(buf[(0, 0)].symbol(), "E");
        assert_eq!(buf[(1, 0)].symbol(), "T");
        assert_eq!(buf[(2, 0)].symbol(), "B");
    }
}
