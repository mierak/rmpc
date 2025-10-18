use std::borrow::Cow;

use anyhow::Result;
use bon::bon;
use itertools::Itertools;
use ratatui::{
    Frame,
    prelude::{Constraint, Layout},
    style::Style,
    symbols::border,
    text::Line,
    widgets::{Block, Borders, Clear},
};

use super::{BUTTON_GROUP_SYMBOLS, Modal, RectExt};
use crate::{
    config::{
        Size,
        keys::{CommonAction, GlobalAction},
    },
    ctx::Ctx,
    shared::{
        id::{self, Id},
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::widgets::button::{Button, ButtonGroup, ButtonGroupState},
};

pub struct ConfirmModal<'a> {
    id: Id,
    message: Vec<Cow<'a, str>>,
    button_group_state: ButtonGroupState,
    button_group: ButtonGroup<'a>,
    size: Option<Size>,
    action: Action<'a>,
}

impl std::fmt::Debug for ConfirmModal<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ConfirmModal(message = {:?}, button_group = {:?}, button_group_state = {:?})",
            self.message, self.button_group, self.button_group_state,
        )
    }
}

type Callback<'a> = Box<dyn FnOnce(&Ctx) -> Result<()> + Send + Sync + 'a>;

pub enum Action<'a> {
    Single {
        confirm_label: Option<&'a str>,
        cancel_label: Option<&'a str>,
        on_confirm: Callback<'a>,
    },
    CustomButtons {
        buttons: Vec<(&'a str, Callback<'a>)>,
    },
}

impl Default for Action<'_> {
    fn default() -> Self {
        Self::CustomButtons { buttons: Vec::default() }
    }
}

#[allow(dead_code)]
#[bon]
impl<'a> ConfirmModal<'a> {
    #[builder]
    #[builder(on(Size, into))]
    pub fn new(
        ctx: &Ctx,
        size: Option<Size>,
        message: Vec<impl Into<Cow<'a, str>>>,
        action: Action<'a>,
    ) -> Self {
        let mut button_group_state = ButtonGroupState::default();

        let buttons = match &action {
            Action::Single { confirm_label, cancel_label, on_confirm: _ } => {
                vec![
                    Button::default().label(confirm_label.unwrap_or("Confirm")),
                    Button::default().label(cancel_label.unwrap_or("Cancel")),
                ]
            }
            Action::CustomButtons { buttons } => {
                buttons.iter().map(|b| Button::default().label(b.0)).collect()
            }
        };

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
            id: id::new(),
            message: message.into_iter().map(|line| line.into()).collect(),
            button_group_state,
            button_group,
            action,
            size,
        }
    }
}

impl Modal for ConfirmModal<'_> {
    fn id(&self) -> Id {
        self.id
    }

    fn render(&mut self, frame: &mut Frame, ctx: &mut Ctx) -> Result<()> {
        let width = match (frame.area().width, self.size) {
            (fw, Some(Size { width, .. })) => width.min(fw),
            (fw, None) if fw > 120 => fw / 2,
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

        let block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_set(border::ROUNDED)
            .border_style(ctx.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center);

        let [content_area, buttons_area] =
            Layout::vertical([Constraint::Min(2), Constraint::Length(3)]).areas(popup_area);

        let areas = Layout::vertical((0..lines.len()).map(|_| Constraint::Length(1)))
            .split(block.inner(content_area));
        frame.render_widget(&block, content_area);

        for (idx, message) in lines.iter().enumerate() {
            // TODO centered default
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
        if let Some(action) = key.as_common_action(ctx) {
            match action {
                CommonAction::Right => {
                    self.button_group_state.next();
                    ctx.render()?;
                }
                CommonAction::Left => {
                    self.button_group_state.prev();
                    ctx.render()?;
                }
                CommonAction::Close => {
                    self.hide(ctx)?;
                }
                CommonAction::Confirm => {
                    match std::mem::take(&mut self.action) {
                        Action::Single { on_confirm, .. } => {
                            if self.button_group_state.selected == 0 {
                                (on_confirm)(ctx)?;
                            }
                        }
                        Action::CustomButtons { mut buttons } => {
                            (buttons.remove(self.button_group_state.selected).1)(ctx)?;
                        }
                    }

                    self.hide(ctx)?;
                }
                _ => {}
            }
        } else if let Some(action) = key.as_global_action(ctx) {
            match action {
                GlobalAction::NextTab => {
                    self.button_group_state.next();
                    ctx.render()?;
                }
                GlobalAction::PreviousTab => {
                    self.button_group_state.prev();
                    ctx.render()?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &mut Ctx) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick => {
                if let Some(idx) = self.button_group.get_button_idx_at(event.into()) {
                    self.button_group_state.select(idx);
                    ctx.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                match std::mem::take(&mut self.action) {
                    Action::Single { on_confirm, .. } => {
                        match self.button_group.get_button_idx_at(event.into()) {
                            Some(0) => {
                                (on_confirm)(ctx)?;
                            }
                            Some(_) => {}
                            None => {}
                        }
                    }
                    Action::CustomButtons { mut buttons } => {
                        (buttons.remove(self.button_group_state.selected).1)(ctx)?;
                    }
                }
                self.hide(ctx)?;
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollUp => {
                if self.button_group.get_button_idx_at(event.into()).is_some() {
                    self.button_group_state.prev();
                    ctx.render()?;
                }
            }
            MouseEventKind::ScrollDown => {
                if self.button_group.get_button_idx_at(event.into()).is_some() {
                    self.button_group_state.next();
                    ctx.render()?;
                }
            }
            MouseEventKind::Drag { drag_start_position: _ } => {}
        }
        Ok(())
    }
}
