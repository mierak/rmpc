use ratatui::{
    prelude::{Constraint, Layout, Margin},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::{ctx::Ctx, ui::input::BufferId};

#[derive(Debug)]
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
    ctx: &'a Ctx,
    buffer_id: Option<BufferId>,
}

impl Input<'_> {
    pub fn new(ctx: &Ctx, buffer_id: BufferId) -> Input<'_> {
        Input {
            ctx,
            buffer_id: Some(buffer_id),
            text: "",
            placeholder: None,
            label: "",
            label_style: Style::default(),
            input_style: Style::default(),
            focused: false,
            focused_style: Style::default(),
            unfocused_style: Style::default(),
            borderless: false,
            spacing: 0,
        }
    }

    pub fn new_static(ctx: &Ctx) -> Input<'_> {
        Input {
            ctx,
            buffer_id: None,
            text: "",
            placeholder: None,
            label: "",
            label_style: Style::default(),
            input_style: Style::default(),
            focused: false,
            focused_style: Style::default(),
            unfocused_style: Style::default(),
            borderless: false,
            spacing: 0,
        }
    }
}

impl Widget for Input<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let label_len = self.label.chars().count() as u16;
        let label_len = if label_len > 0 { label_len + 2 } else { 0 };
        let [text_area, input_area] =
            *Layout::horizontal([Constraint::Max(label_len), Constraint::Fill(1)])
                .spacing(self.spacing)
                .split(area)
        else {
            return;
        };

        let input_area = input_area.inner(Margin { horizontal: 0, vertical: 0 });

        let block_border_style =
            if self.focused { self.focused_style } else { self.unfocused_style };

        let label = Paragraph::new(self.label).wrap(Wrap { trim: false }).style(self.label_style);

        let mut text =
            self.buffer_id.map_or(vec![Span::styled(self.text, self.input_style)], |id| {
                let is_active = self.ctx.input.is_active(id);
                self.ctx.input.as_spans(
                    id,
                    input_area.width.saturating_sub(if self.borderless { 0 } else { 2 }),
                    self.input_style,
                    is_active,
                )
            });

        if let Some(pl) = self.placeholder
            && (text.is_empty() || text.iter().all(|s| s.content.is_empty()))
        {
            text.clear();
            text.push(Span::styled(pl, self.input_style));
        }
        let mut input = Paragraph::new(Line::from(text)).style(self.input_style);

        if !self.borderless {
            input = input.block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(ratatui::symbols::border::ROUNDED)
                    .border_style(block_border_style),
            );
        }

        input.render(input_area, buf);
        label.render(
            text_area.inner(Margin { horizontal: 0, vertical: u16::from(!self.borderless) }),
            buf,
        );
    }
}

#[allow(unused)]
impl<'a> Input<'a> {
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

    pub fn buffer_id(mut self, buffer_id: BufferId) -> Self {
        self.buffer_id = Some(buffer_id);
        self
    }

    pub fn ctx(mut self, ctx: &'a Ctx) -> Self {
        self.ctx = ctx;
        self
    }
}
