use bon::bon;
use ratatui::{
    prelude::{Constraint, Layout, Margin},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::{ctx::Ctx, ui::input::BufferId};

#[derive(Debug)]
pub struct Input<'a> {
    ctx: &'a Ctx,
    buffer_id: Option<BufferId>,

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
}

#[bon]
impl<'a> Input<'a> {
    #[builder]
    pub fn new(
        ctx: &'a Ctx,
        buffer_id: BufferId,
        placeholder: Option<&'a str>,
        label: &'a str,
        #[builder(default)] label_style: Style,
        #[builder(default)] input_style: Style,
        #[builder(default)] focused: bool,
        #[builder(default)] focused_style: Style,
        #[builder(default)] unfocused_style: Style,
        #[builder(default)] borderless: bool,
        #[builder(default)] spacing: u16,
    ) -> Input<'a> {
        Input {
            ctx,
            buffer_id: Some(buffer_id),
            text: "",
            placeholder,
            label,
            label_style,
            input_style,
            focused,
            focused_style,
            unfocused_style,
            borderless,
            spacing,
        }
    }

    #[builder]
    pub fn new_static(
        ctx: &'a Ctx,
        text: &'a str,
        placeholder: Option<&'a str>,
        label: &'a str,
        #[builder(default)] label_style: Style,
        #[builder(default)] input_style: Style,
        #[builder(default)] focused: bool,
        #[builder(default)] focused_style: Style,
        #[builder(default)] unfocused_style: Style,
        #[builder(default)] borderless: bool,
        #[builder(default)] spacing: u16,
    ) -> Input<'a> {
        Input {
            ctx,
            buffer_id: None,
            text,
            placeholder,
            label,
            label_style,
            input_style,
            focused,
            focused_style,
            unfocused_style,
            borderless,
            spacing,
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
