use std::borrow::Cow;

use anyhow::Result;
use bon::bon;
use itertools::Itertools;
use ratatui::{
    Frame,
    layout::Alignment,
    prelude::{Constraint, Layout},
    style::Style,
    symbols::border,
    text::Line,
    widgets::{Block, Borders, Clear},
};

use super::{BUTTON_GROUP_SYMBOLS, Modal, RectExt};
use crate::{
    config::{Size, keys::CommonAction},
    ctx::Ctx,
    shared::{
        key_event::KeyEvent,
        macros::pop_modal,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::widgets::button::{Button, ButtonGroup, ButtonGroupState},
};

#[derive(Debug)]
pub struct InfoModal<'a> {
    message: Vec<String>,
    button_group_state: ButtonGroupState,
    button_group: ButtonGroup<'a>,
    id: Option<Cow<'static, str>>,
    size: Option<Size>,
    title: Option<Cow<'a, str>>,
}

#[allow(dead_code)]
#[bon]
impl<'a> InfoModal<'a> {
    #[builder]
    pub fn new(
        ctx: &Ctx,
        size: Option<impl Into<Size>>,
        confirm_label: Option<&'a str>,
        message: Vec<String>,
        id: Option<impl Into<Cow<'static, str>>>,
        title: Option<impl Into<Cow<'a, str>>>,
    ) -> Self {
        let mut button_group_state = ButtonGroupState::default();
        let buttons = vec![Button::default().label(confirm_label.unwrap_or("Ok"))];
        button_group_state.set_button_count(buttons.len());
        let button_group = ButtonGroup::default()
            .active_style(ctx.config.theme.current_item_style)
            .inactive_style(ctx.config.as_text_style())
            .buttons(buttons)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(BUTTON_GROUP_SYMBOLS)
                    .border_style(ctx.config.as_border_style()),
            );

        Self {
            message,
            button_group_state,
            button_group,
            size: size.map(|s| s.into()),
            title: title.map(|v| v.into()),
            id: id.map(|i| i.into()),
        }
    }
}

impl Modal for InfoModal<'_> {
    fn render(&mut self, frame: &mut Frame, ctx: &mut Ctx) -> Result<()> {
        let width = match (frame.area().width, self.size) {
            (fw, Some(Size { width, .. })) => width.min(fw),
            (fw, None) if fw > 60 => fw / 2,
            (fw, None) => fw,
        };

        let lines = self
            .message
            .iter()
            .flat_map(|message| message.lines())
            .flat_map(|line| textwrap::wrap(line, (width as usize).saturating_sub(2)))
            .collect_vec();

        let popup_area = frame
            .area()
            .centered_exact(width, self.size.map_or(u16::try_from(lines.len())? + 4, |v| v.height));
        frame.render_widget(Clear, popup_area);

        if let Some(bg_color) = ctx.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let mut block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_set(border::ROUNDED)
            .border_style(ctx.config.as_border_style())
            .title_alignment(Alignment::Left);
        if let Some(title) = &self.title {
            block = block.title(title.as_ref());
        }

        let [content_area, buttons_area] =
            Layout::vertical([Constraint::Min(2), Constraint::Length(3)]).areas(popup_area);

        let areas = Layout::vertical((0..lines.len()).map(|_| Constraint::Length(1)))
            .split(block.inner(content_area));
        frame.render_widget(&block, content_area);

        for (idx, message) in lines.iter().enumerate() {
            let paragraph =
                Line::from(message.as_ref()).style(ctx.config.as_text_style()).left_aligned();

            let Some(area) = areas.get(idx) else {
                continue;
            };
            frame.render_widget(paragraph, *area);
        }

        frame.render_stateful_widget(
            &mut self.button_group,
            buttons_area,
            &mut self.button_group_state,
        );
        Ok(())
    }

    fn handle_key(&mut self, key: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        if let Some(CommonAction::Close | CommonAction::Confirm) = key.as_common_action(ctx) {
            pop_modal!(ctx);
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &mut Ctx) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick => {
                if let Some(idx) = self.button_group.get_button_idx_at(event.into()) {
                    self.button_group_state.select(idx);
                    pop_modal!(ctx);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn get_id(&self) -> Option<Cow<'static, str>> {
        self.id.clone()
    }
}
