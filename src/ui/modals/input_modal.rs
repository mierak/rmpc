use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    prelude::{Constraint, Layout},
    style::{Style, Stylize},
    symbols::border,
    widgets::{Block, Borders, Clear},
};

use super::{BUTTON_GROUP_SYMBOLS, Modal, RectExt};
use crate::{
    config::keys::CommonAction,
    ctx::Ctx,
    shared::{
        id::{self, Id},
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::widgets::{
        button::{Button, ButtonGroup, ButtonGroupState},
        input::Input,
    },
};

pub struct InputModal<'a, C: FnOnce(&Ctx, &str) -> Result<()> + 'a> {
    id: Id,
    button_group_state: ButtonGroupState,
    button_group: ButtonGroup<'a>,
    input_focused: bool,
    input_area: Rect,
    callback: Option<C>,
    value: String,
    title: &'a str,
    input_label: &'a str,
}

impl<Callback: FnOnce(&Ctx, &str) -> Result<()>> std::fmt::Debug for InputModal<'_, Callback> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "InputModal(message = {}, button_group = {:?}, button_group_state = {:?})",
            self.input_label, self.button_group, self.button_group_state,
        )
    }
}

impl<'a, C: FnOnce(&Ctx, &str) -> Result<()> + 'a> InputModal<'a, C> {
    pub fn new(ctx: &Ctx) -> Self {
        let mut button_group_state = ButtonGroupState::default();
        let buttons = vec![Button::default().label("Save"), Button::default().label("Cancel")];
        button_group_state.set_button_count(buttons.len());

        let button_group = ButtonGroup::default()
            .buttons(buttons)
            .inactive_style(ctx.config.as_text_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(BUTTON_GROUP_SYMBOLS)
                    .border_style(ctx.config.as_border_style()),
            );

        Self {
            id: id::new(),
            button_group_state,
            button_group,
            input_focused: true,
            input_area: Rect::default(),
            callback: None,
            value: String::new(),
            input_label: "",
            title: "",
        }
    }

    pub fn confirm_label(mut self, label: &'a str) -> Self {
        let buttons = vec![Button::default().label(label), Button::default().label("Cancel")];
        self.button_group = self.button_group.buttons(buttons);
        self
    }

    pub fn on_confirm(mut self, callback: C) -> Self {
        self.callback = Some(callback);
        self
    }

    pub fn input_label(mut self, label: &'a str) -> Self {
        self.input_label = label;
        self
    }

    pub fn title(mut self, message: &'a str) -> Self {
        self.title = message;
        self
    }

    pub fn initial_value(mut self, value: String) -> Self {
        self.value = value;
        self
    }
}

impl<'a, C: FnOnce(&Ctx, &str) -> Result<()> + 'a> Modal for InputModal<'a, C> {
    fn id(&self) -> Id {
        self.id
    }

    fn render(&mut self, frame: &mut Frame, ctx: &mut Ctx) -> Result<()> {
        let block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_set(border::ROUNDED)
            .border_style(ctx.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center)
            .title(self.title);

        let popup_area = frame.area().centered_exact(50, 7);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = ctx.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }
        let [body_area, buttons_area] =
            *Layout::vertical([Constraint::Length(4), Constraint::Max(3)]).split(popup_area)
        else {
            return Ok(());
        };

        let input = Input::default()
            .set_label(self.input_label)
            .set_label_style(ctx.config.as_text_style())
            .set_text(&self.value)
            .set_focused(self.input_focused)
            .set_focused_style(ctx.config.theme.highlight_border_style)
            .set_unfocused_style(ctx.config.as_border_style());

        self.button_group.set_active_style(if self.input_focused {
            Style::default().reversed()
        } else {
            ctx.config.theme.current_item_style
        });

        self.input_area = body_area;

        frame.render_widget(input, block.inner(body_area));
        frame.render_widget(block, body_area);
        frame.render_stateful_widget(
            &mut self.button_group,
            buttons_area,
            &mut self.button_group_state,
        );
        Ok(())
    }

    fn handle_key(&mut self, key: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        let action = key.as_common_action(ctx);
        if self.input_focused {
            if let Some(CommonAction::Close) = action {
                self.input_focused = false;

                ctx.render()?;
                return Ok(());
            } else if let Some(CommonAction::Confirm) = action {
                if self.button_group_state.selected == 0 {
                    if let Some(callback) = self.callback.take() {
                        (callback)(ctx, &self.value)?;
                    }
                }
                self.hide(ctx)?;
                return Ok(());
            }

            match key.code() {
                KeyCode::Char(c) => {
                    self.value.push(c);

                    ctx.render()?;
                }
                KeyCode::Backspace => {
                    self.value.pop();

                    ctx.render()?;
                }
                _ => {}
            }
        } else if let Some(action) = action {
            match action {
                CommonAction::Down => {
                    self.button_group_state.next();

                    ctx.render()?;
                }
                CommonAction::Up => {
                    self.button_group_state.next();

                    ctx.render()?;
                }
                CommonAction::Close => {
                    self.hide(ctx)?;
                }
                CommonAction::Confirm => {
                    if self.button_group_state.selected == 0 {
                        if let Some(callback) = self.callback.take() {
                            (callback)(ctx, &self.value)?;
                        }
                    }
                    self.hide(ctx)?;
                }
                CommonAction::FocusInput => {
                    self.input_focused = true;

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
                    self.input_focused = false;
                    ctx.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                match self.button_group.get_button_idx_at(event.into()) {
                    Some(0) => {
                        if let Some(callback) = self.callback.take() {
                            (callback)(ctx, &self.value)?;
                        }
                        self.hide(ctx)?;
                    }
                    Some(_) => {
                        self.hide(ctx)?;
                    }
                    None => {
                        if self.input_area.contains(event.into()) {
                            self.input_focused = true;
                            ctx.render()?;
                        }
                    }
                }
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollUp => {
                if self.button_group.get_button_idx_at(event.into()).is_some() {
                    self.input_focused = false;
                    self.button_group_state.prev();
                    ctx.render()?;
                }
            }
            MouseEventKind::ScrollDown => {
                if self.button_group.get_button_idx_at(event.into()).is_some() {
                    self.input_focused = false;
                    self.button_group_state.next();
                    ctx.render()?;
                }
            }
            MouseEventKind::Drag { drag_start_position: _ } => {}
        }
        Ok(())
    }
}
