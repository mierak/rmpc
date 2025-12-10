use anyhow::Result;
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
    ui::{
        input::{BufferId, InputResultEvent},
        widgets::{
            button::{Button, ButtonGroup, ButtonGroupState},
            input::Input,
        },
    },
};

pub struct InputModal<'a, C: FnOnce(&Ctx, &str) -> Result<()> + 'a> {
    id: Id,
    button_group_state: ButtonGroupState,
    button_group: ButtonGroup<'a>,
    input_area: Rect,
    callback: Option<C>,
    initial_value: String,
    title: &'a str,
    input_label: &'a str,
    input_buffer_id: BufferId,
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

        let input_buffer_id = BufferId::new();
        ctx.insert_mode(input_buffer_id);

        Self {
            id: id::new(),
            button_group_state,
            button_group,
            input_area: Rect::default(),
            callback: None,
            initial_value: String::new(),
            input_label: "",
            title: "",
            input_buffer_id,
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
        self.initial_value = value;
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

        let input = Input::new(ctx, self.input_buffer_id)
            .set_label(self.input_label)
            .set_label_style(ctx.config.as_text_style())
            .set_focused(ctx.input.is_insert_mode())
            .set_focused_style(ctx.config.theme.highlight_border_style)
            .set_unfocused_style(ctx.config.as_border_style());

        self.button_group.set_active_style(if ctx.input.is_active(self.input_buffer_id) {
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

    fn handle_insert_mode(&mut self, kind: InputResultEvent, ctx: &Ctx) -> Result<()> {
        match kind {
            InputResultEvent::Push => {}
            InputResultEvent::Pop => {}
            InputResultEvent::Confirm => {
                if self.button_group_state.selected == 0
                    && let Some(callback) = self.callback.take()
                {
                    (callback)(ctx, &ctx.input.value(self.input_buffer_id))?;
                }
                ctx.input.destroy_buffer(self.input_buffer_id);
                self.hide(ctx)?;
            }
            InputResultEvent::NoChange => {}
            InputResultEvent::Cancel => {}
        }
        ctx.render()?;
        Ok(())
    }

    fn handle_key(&mut self, key: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        if let Some(action) = key.as_common_action(ctx) {
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
                    ctx.input.destroy_buffer(self.input_buffer_id);
                    self.hide(ctx)?;
                }
                CommonAction::Confirm => {
                    if self.button_group_state.selected == 0
                        && let Some(callback) = self.callback.take()
                    {
                        (callback)(ctx, &ctx.input.value(self.input_buffer_id))?;
                    }
                    ctx.input.destroy_buffer(self.input_buffer_id);
                    self.hide(ctx)?;
                }
                CommonAction::FocusInput => {
                    ctx.insert_mode(self.input_buffer_id);
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
                    ctx.input.normal_mode();
                    ctx.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                match self.button_group.get_button_idx_at(event.into()) {
                    Some(0) => {
                        if let Some(callback) = self.callback.take() {
                            (callback)(ctx, &ctx.input.value(self.input_buffer_id))?;
                        }
                        ctx.input.destroy_buffer(self.input_buffer_id);
                        self.hide(ctx)?;
                    }
                    Some(_) => {
                        ctx.input.destroy_buffer(self.input_buffer_id);
                        self.hide(ctx)?;
                    }
                    None => {
                        if self.input_area.contains(event.into()) {
                            ctx.input.insert_mode(self.input_buffer_id);
                            ctx.render()?;
                        }
                    }
                }
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollUp => {
                if self.button_group.get_button_idx_at(event.into()).is_some() {
                    ctx.input.normal_mode();
                    self.button_group_state.prev();
                    ctx.render()?;
                }
            }
            MouseEventKind::ScrollDown => {
                if self.button_group.get_button_idx_at(event.into()).is_some() {
                    ctx.input.normal_mode();
                    self.button_group_state.next();
                    ctx.render()?;
                }
            }
            MouseEventKind::Drag { drag_start_position: _ } => {}
        }
        Ok(())
    }
}
