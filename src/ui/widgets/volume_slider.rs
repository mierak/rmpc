use ratatui::{
    prelude::{Buffer, Rect},
    style::{Color, Style},
    widgets::Widget,
};

#[derive(Clone)]
pub struct VolumeSlider<'a> {
    value: f64,
    start_char: &'a str,
    filled_char: &'a str,
    thumb_char: &'a str,
    empty_char: &'a str,
    end_char: &'a str,
    filled_style: Style,
    thumb_style: Style,
    empty_style: Style,
}

#[allow(dead_code)]
impl<'a> VolumeSlider<'a> {
    pub fn value(mut self, val: f64) -> Self {
        self.value = val.clamp(0.0, 1.0);
        self
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.filled_style = self.filled_style.fg(color);
        self.thumb_style = self.thumb_style.fg(color);
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.thumb_style = self.thumb_style.bg(color);
        self.empty_style = self.empty_style.fg(color);
        self
    }

    pub fn start_char(mut self, start: &'a str) -> Self {
        self.start_char = start;
        self
    }

    pub fn filled_char(mut self, filled: &'a str) -> Self {
        self.filled_char = filled;
        self
    }

    pub fn thumb_char(mut self, thumb: &'a str) -> Self {
        self.thumb_char = thumb;
        self
    }

    pub fn empty_char(mut self, empty: &'a str) -> Self {
        self.empty_char = empty;
        self
    }

    pub fn end_char(mut self, end: &'a str) -> Self {
        self.end_char = end;
        self
    }

    pub fn filled_style(mut self, style: Style) -> Self {
        self.filled_style = style;
        self
    }

    pub fn thumb_style(mut self, style: Style) -> Self {
        self.thumb_style = style;
        self
    }

    pub fn empty_style(mut self, style: Style) -> Self {
        self.empty_style = style;
        self
    }
}

impl Widget for VolumeSlider<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 || area.width < 1 {
            return;
        }

        let left = area.left();
        let right = area.right();
        let top = area.top();

        let len = right.saturating_sub(left);

        buf.set_string(left, top, self.empty_char.repeat(len as usize), self.empty_style);

        let filled_len = (f64::from(len) * self.value) as usize;

        if filled_len > 0 {
            buf.set_string(left, top, self.filled_char.repeat(filled_len), self.filled_style);

            if filled_len < (len.saturating_sub(1)) as usize {
                buf.set_string(left + filled_len as u16, top, self.thumb_char, self.thumb_style);
            }
        }

        buf.set_string(
            left,
            top,
            self.start_char,
            if filled_len > 0 { self.filled_style } else { self.empty_style },
        );

        buf.set_string(
            right.saturating_sub(1),
            top,
            self.end_char,
            if filled_len >= (len.saturating_sub(1)) as usize {
                self.filled_style
            } else {
                self.empty_style
            },
        );
    }
}

impl Default for VolumeSlider<'_> {
    fn default() -> Self {
        Self {
            value: 0.0,
            start_char: "♪",
            filled_char: "─",
            thumb_char: "●",
            empty_char: "─",
            end_char: "♫",
            filled_style: Style::default().fg(Color::Blue),
            thumb_style: Style::default().bg(Color::Black).fg(Color::Blue),
            empty_style: Style::default().fg(Color::Black),
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

    use super::VolumeSlider;

    #[test]
    fn volume_zero_is_correct() {
        let wg = VolumeSlider {
            start_char: "♪",
            filled_char: "█",
            thumb_char: "●",
            empty_char: "─",
            end_char: "♪",
            ..Default::default()
        }
        .value(0.0);
        let area = Rect::new(0, 0, 5, 1);
        let mut buf = Buffer { area, content: vec![Cell::default(); 5] };

        wg.render(area, &mut buf);

        assert_eq!(buf[(0, 0)].symbol(), "♪");
        assert_eq!(buf[(1, 0)].symbol(), "─");
        assert_eq!(buf[(2, 0)].symbol(), "─");
        assert_eq!(buf[(3, 0)].symbol(), "─");
        assert_eq!(buf[(4, 0)].symbol(), "♪");
    }

    #[test]
    fn volume_max_is_correct() {
        let wg = VolumeSlider {
            start_char: "♪",
            filled_char: "█",
            thumb_char: "●",
            empty_char: "─",
            end_char: "♪",
            ..Default::default()
        }
        .value(1.0);
        let area = Rect::new(0, 0, 5, 1);
        let mut buf = Buffer { area, content: vec![Cell::default(); 5] };

        wg.render(area, &mut buf);

        assert_eq!(buf[(0, 0)].symbol(), "♪");
        assert_eq!(buf[(1, 0)].symbol(), "█");
        assert_eq!(buf[(2, 0)].symbol(), "█");
        assert_eq!(buf[(3, 0)].symbol(), "█");
        assert_eq!(buf[(4, 0)].symbol(), "♪");
    }

    #[test]
    fn volume_half_is_correct() {
        let wg = VolumeSlider {
            start_char: "♪",
            filled_char: "█",
            thumb_char: "●",
            empty_char: "─",
            end_char: "♪",
            ..Default::default()
        }
        .value(0.5);
        let area = Rect::new(0, 0, 5, 1);
        let mut buf = Buffer { area, content: vec![Cell::default(); 5] };

        wg.render(area, &mut buf);

        assert_eq!(buf[(0, 0)].symbol(), "♪");
        assert_eq!(buf[(1, 0)].symbol(), "█");
        assert_eq!(buf[(2, 0)].symbol(), "●");
        assert_eq!(buf[(3, 0)].symbol(), "─");
        assert_eq!(buf[(4, 0)].symbol(), "♪");
    }

    #[test]
    fn volume_slider_clamps_values() {
        let wg = VolumeSlider::default().value(-0.5);
        assert!((wg.value - 0.0).abs() < 1e-6);

        let wg = VolumeSlider::default().value(1.5);
        assert!((wg.value - 1.0).abs() < 1e-6);
    }
}
