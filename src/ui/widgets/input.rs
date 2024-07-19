use std::borrow::Cow;

use ratatui::{
    prelude::{Constraint, Layout, Margin},
    style::Style,
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

#[derive(Debug, Default)]
pub struct Input<'a> {
    text: &'a str,
    placeholder: Option<&'a str>,
    label: &'a str,
    label_style: Style,
    input_style: Style,
    focused: bool,
    focused_style: Style,
    unfocused_style: Style,
    borderless: bool,
}

impl Widget for Input<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let label_len = self.label.chars().count() as u16;
        let [text_area, input_area] =
            *Layout::horizontal([Constraint::Max(label_len + 2), Constraint::Fill(1)]).split(area)
        else {
            return;
        };

        let input_area = input_area.inner(Margin {
            horizontal: 0,
            vertical: 0,
        });

        let block_border_style = if self.focused {
            self.focused_style
        } else {
            self.unfocused_style
        };

        let label = Paragraph::new(self.label)
            .wrap(Wrap { trim: false })
            .style(self.label_style);
        let mut input = Paragraph::new(self.trimed_text(input_area)).style(self.input_style);

        if !self.borderless {
            input = input.block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(ratatui::symbols::border::ROUNDED)
                    .border_style(block_border_style),
            );
        }

        input = input.wrap(Wrap { trim: true });

        label.render(
            text_area.inner(Margin {
                horizontal: 0,
                vertical: u16::from(!self.borderless),
            }),
            buf,
        );
        input.render(input_area, buf);
    }
}

#[allow(unused)]
impl<'a> Input<'a> {
    fn trimed_text(&self, input_area: ratatui::layout::Rect) -> Cow<'a, str> {
        if self.text.is_empty() && !self.focused {
            return Cow::Borrowed(self.placeholder.unwrap_or(""));
        }

        let mut input_len = input_area
            .inner(Margin {
                horizontal: 1,
                vertical: 0,
            })
            .width as usize;

        if self.focused {
            input_len = input_len.saturating_sub(1);
        }

        Cow::Owned(format!(
            "{}{}",
            self.text
                .chars()
                .skip(self.text.len().saturating_sub(input_len))
                .collect::<String>(),
            if self.focused { "█" } else { "" },
        ))
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

    pub fn set_borderless(mut self, borderless: bool) -> Self {
        self.borderless = borderless;
        self
    }

    pub fn set_label_style(mut self, label_style: Style) -> Self {
        self.label_style = label_style;
        self
    }

    pub fn set_input_style(mut self, input_style: Style) -> Self {
        self.input_style = input_style;
        self
    }

    pub fn set_placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = Some(placeholder);
        self
    }
}
