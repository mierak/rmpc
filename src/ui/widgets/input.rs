use itertools::Itertools;
use ratatui::{
    prelude::{Constraint, Layout, Margin},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};
use unicode_segmentation::UnicodeSegmentation;

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
    spacing: u16,
    cursor: usize,
}

impl Widget for Input<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let label_len = self.label.chars().count() as u16;
        let [text_area, input_area] =
            // TODO when no label it takes 2 columns, fix that
            *Layout::horizontal([Constraint::Max(label_len + 2), Constraint::Fill(1)])
                .spacing(self.spacing)
                .split(area)
        else {
            return;
        };

        let input_area = input_area.inner(Margin { horizontal: 0, vertical: 0 });

        let block_border_style =
            if self.focused { self.focused_style } else { self.unfocused_style };

        let label = Paragraph::new(self.label).wrap(Wrap { trim: false }).style(self.label_style);
        let text = self.trimmed_text(input_area);
        let text_width = text.iter().map(|span| span.width()).sum1::<usize>().unwrap_or_default();

        log::debug!(area = input_area.width; "Input widget text width: {text_width}");
        // TODO account for borders
        let mut input = if text_width + 2 <= input_area.width as usize {
            Paragraph::new(Line::from(text))
        } else {
            Paragraph::new(Line::from(""))
        };

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
            text_area.inner(Margin { horizontal: 0, vertical: u16::from(!self.borderless) }),
            buf,
        );
        input.render(input_area, buf);
    }
}

#[allow(unused)]
impl<'a> Input<'a> {
    fn trimmed_text(&self, input_area: ratatui::layout::Rect) -> Vec<Span<'a>> {
        if self.text.is_empty() && !self.focused {
            return vec![Span::default()];
        }

        let mut input_len = input_area.inner(Margin { horizontal: 1, vertical: 0 }).width as usize;

        if self.focused {
            input_len = input_len.saturating_sub(1);
        }

        if self.text.len() == self.cursor {
            return vec![
                Span::styled(&self.text[0..self.cursor], self.input_style),
                Span::styled(" ", self.input_style.reversed()),
            ];
        }

        let grapheme_size = self
            .text
            .grapheme_indices(true)
            .find(|(idx, _)| idx >= &self.cursor)
            .map_or(self.text.len(), |(idx, g)| g.len());

        return vec![
            Span::styled(&self.text[0..self.cursor], self.input_style),
            Span::styled(
                &self.text[self.cursor..self.cursor + grapheme_size],
                self.input_style.reversed(),
            ),
            Span::styled(&self.text[self.cursor + grapheme_size..], self.input_style),
        ];
    }

    pub fn spacing(mut self, spacing: u16) -> Self {
        self.spacing = spacing;
        self
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

    pub fn cursor(mut self, position: usize) -> Self {
        self.cursor = position;
        self
    }
}
