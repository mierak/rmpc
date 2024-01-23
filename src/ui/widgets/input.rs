use ratatui::{
    prelude::{Constraint, Direction, Layout, Margin},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

#[derive(Debug, Default)]
pub struct Input<'a> {
    text: &'a str,
    label: &'a str,
    focused: bool,
    focused_style: Style,
    unfocused_style: Style,
}

impl Widget for Input<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let [text_area, input_area] = *Layout::default()
            .constraints(
                [
                    Constraint::Max(self.label.chars().count() as u16 + 2),
                    Constraint::Max(24),
                ]
                .as_ref(),
            )
            .direction(Direction::Horizontal)
            .split(area)
        else {
            return;
        };

        let input_area = input_area.inner(&Margin {
            horizontal: 1,
            vertical: 0,
        });

        let block_border_style = if self.focused {
            self.focused_style
        } else {
            self.unfocused_style
        };
        let label = Paragraph::new(self.label).wrap(Wrap { trim: true });
        let input = Paragraph::new(self.trimed_text(input_area))
            .block(Block::default().borders(Borders::ALL).border_style(block_border_style))
            .fg(Color::White)
            .wrap(Wrap { trim: true });

        label.render(
            text_area.inner(&Margin {
                horizontal: 1,
                vertical: 1,
            }),
            buf,
        );
        input.render(input_area, buf);
    }
}

#[allow(unused)]
impl<'a> Input<'a> {
    fn trimed_text(&self, input_area: ratatui::layout::Rect) -> String {
        let mut input_len = input_area
            .inner(&Margin {
                horizontal: 1,
                vertical: 0,
            })
            .width as usize;

        if self.focused {
            input_len = input_len.saturating_sub(1);
        }

        format!(
            "{}{}",
            self.text
                .chars()
                .skip(self.text.len().saturating_sub(input_len))
                .collect::<String>(),
            if self.focused { "█" } else { "" },
        )
    }
    pub fn set_text(mut self, text: &'a str) -> Self {
        self.text = text;
        self
    }

    pub fn set_label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    pub fn set_focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn set_focused_style(mut self, focused_style: Style) -> Self {
        self.focused_style = focused_style;
        self
    }

    pub fn set_unfocused_style(mut self, unfocused_style: Style) -> Self {
        self.unfocused_style = unfocused_style;
        self
    }
}
