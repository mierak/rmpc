use bon::Builder;
use ratatui::{
    prelude::{Buffer, Rect},
    style::{Color, Style},
    widgets::Widget,
};

#[derive(Clone, Builder)]
pub struct ProgressBar<'a> {
    value: f32,
    start_char: &'a str,
    elapsed_char: &'a str,
    thumb_char: &'a str,
    track_char: &'a str,
    end_char: &'a str,
    elapsed_style: Style,
    thumb_style: Style,
    track_style: Style,
    use_track_when_empty: bool,
}

impl Widget for ProgressBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 || area.width < 1 {
            return;
        }

        let left = area.left();
        let top = area.top();
        let len = area.width;

        buf.set_string(left, top, self.track_char.repeat(len as usize), self.track_style);

        let filled_cols = ((len as f32 * self.value).round() as u16).min(len);

        for i in 0..len {
            let x = left + i;
            let last_idx = len.saturating_sub(1);

            let (char, style) = if i == 0 && self.use_track_when_empty && filled_cols == 0 {
                // start char
                (self.track_char, self.track_style)
            } else if i == last_idx && self.use_track_when_empty && filled_cols < last_idx {
                // end char
                (self.track_char, self.track_style)
            } else if i == 0 {
                // start char
                let style = if filled_cols == 0 { self.track_style } else { self.elapsed_style };
                (self.start_char, style)
            } else if i == last_idx {
                // end char
                let style =
                    if filled_cols < last_idx { self.track_style } else { self.elapsed_style };
                (self.end_char, style)
            } else if i == filled_cols {
                // thumb
                (self.thumb_char, self.thumb_style)
            } else if i < filled_cols {
                // elapsed
                (self.elapsed_char, self.elapsed_style)
            } else {
                // track
                (self.track_char, self.track_style)
            };

            buf.set_string(x, top, char, style);
        }
    }
}

impl Default for ProgressBar<'_> {
    fn default() -> Self {
        Self {
            value: 0.0,
            start_char: "-",
            elapsed_char: "█",
            thumb_char: "",
            track_char: " ",
            end_char: "═",
            elapsed_style: Style::default().fg(Color::Blue),
            thumb_style: Style::default().bg(Color::Black).fg(Color::Blue),
            track_style: Style::default().bg(Color::Black),
            use_track_when_empty: false,
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
            start_char: "S",
            elapsed_char: "E",
            thumb_char: "T",
            track_char: "B",
            end_char: "E",
            value: 0.0,
            ..Default::default()
        };
        let area = Rect::new(0, 0, 5, 1);
        let mut buf = Buffer { area, content: vec![Cell::default(); 5] };

        wg.render(area, &mut buf);

        assert_eq!(buf[(0, 0)].symbol(), "S");
        assert_eq!(buf[(1, 0)].symbol(), "B");
        assert_eq!(buf[(2, 0)].symbol(), "B");
        assert_eq!(buf[(3, 0)].symbol(), "B");
        assert_eq!(buf[(4, 0)].symbol(), "E");
    }

    #[test]
    fn upper_bound_is_correct() {
        let wg = ProgressBar {
            start_char: "S",
            elapsed_char: "E",
            thumb_char: "T",
            track_char: "B",
            end_char: "E",
            value: 1.0,
            ..Default::default()
        };
        let area = Rect::new(0, 0, 5, 1);
        let mut buf = Buffer { area, content: vec![Cell::default(); 5] };

        wg.render(area, &mut buf);

        assert_eq!(buf[(0, 0)].symbol(), "S");
        assert_eq!(buf[(1, 0)].symbol(), "E");
        assert_eq!(buf[(2, 0)].symbol(), "E");
        assert_eq!(buf[(3, 0)].symbol(), "E");
        assert_eq!(buf[(4, 0)].symbol(), "E");
    }

    #[test]
    fn middle_is_correct() {
        let wg = ProgressBar {
            start_char: "S",
            elapsed_char: "E",
            thumb_char: "T",
            track_char: "B",
            end_char: "X",
            value: 0.49,
            ..Default::default()
        };
        let area = Rect::new(0, 0, 5, 1);
        let mut buf = Buffer { area, content: vec![Cell::default(); 5] };

        wg.render(area, &mut buf);

        assert_eq!(buf[(0, 0)].symbol(), "S");
        assert_eq!(buf[(1, 0)].symbol(), "E");
        assert_eq!(buf[(2, 0)].symbol(), "T");
        assert_eq!(buf[(3, 0)].symbol(), "B");
        assert_eq!(buf[(4, 0)].symbol(), "X");
    }

    #[test]
    fn only_track_when_empty() {
        let wg = ProgressBar {
            start_char: "S",
            elapsed_char: "E",
            thumb_char: "T",
            track_char: "B",
            end_char: "E",
            value: 0.0,
            use_track_when_empty: true,
            ..Default::default()
        };
        let area = Rect::new(0, 0, 5, 1);
        let mut buf = Buffer { area, content: vec![Cell::default(); 5] };

        wg.render(area, &mut buf);

        assert_eq!(buf[(0, 0)].symbol(), "B");
        assert_eq!(buf[(1, 0)].symbol(), "B");
        assert_eq!(buf[(2, 0)].symbol(), "B");
        assert_eq!(buf[(3, 0)].symbol(), "B");
        assert_eq!(buf[(4, 0)].symbol(), "B");
    }
}
