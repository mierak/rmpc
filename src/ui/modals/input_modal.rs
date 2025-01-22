use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    prelude::{Constraint, Layout},
    style::{Style, Stylize},
    symbols::{self, border},
    widgets::{Block, Borders, Clear},
    Frame,
};

use crate::{
    config::keys::CommonAction,
    context::AppContext,
    shared::{
        key_event::KeyEvent,
        macros::pop_modal,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::widgets::{
        button::{Button, ButtonGroup, ButtonGroupState},
        input::Input,
    },
};

use super::RectExt;

use super::Modal;

const BUTTON_GROUP_SYMBOLS: symbols::border::Set = symbols::border::Set {
    top_right: symbols::line::NORMAL.vertical_left,
    top_left: symbols::line::NORMAL.vertical_right,
    ..symbols::border::ROUNDED
};

pub struct InputModal<'a, C: FnMut(&AppContext, &str) -> Result<()> + 'a> {
    button_group_state: ButtonGroupState,
    button_group: ButtonGroup<'a>,
    input_focused: bool,
    input_area: Rect,
    callback: Option<C>,
    value: String,
    title: &'a str,
    input_label: &'a str,
}

impl<Callback: FnMut(&AppContext, &str) -> Result<()>> std::fmt::Debug for InputModal<'_, Callback> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "InputModal(message = {}, button_group = {:?}, button_group_state = {:?})",
            self.input_label, self.button_group, self.button_group_state,
        )
    }
}

impl<'a, C: FnMut(&AppContext, &str) -> Result<()> + 'a> InputModal<'a, C> {
    pub fn new(context: &AppContext) -> Self {
        let mut button_group_state = ButtonGroupState::default();
        let buttons = vec![Button::default().label("Save"), Button::default().label("Cancel")];
        button_group_state.set_button_count(buttons.len());

        let button_group = ButtonGroup::default()
            .buttons(buttons)
            .inactive_style(context.config.as_text_style())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(BUTTON_GROUP_SYMBOLS)
                    .border_style(context.config.as_border_style()),
            );

        Self {
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

impl<'a, C: FnMut(&AppContext, &str) -> Result<()> + 'a> Modal for InputModal<'a, C> {
    fn render(&mut self, frame: &mut Frame, app: &mut AppContext) -> Result<()> {
        let block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_set(border::ROUNDED)
            .border_style(app.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center)
            .title(self.title);

        let popup_area = frame.area().centered_exact(50, 7);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = app.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }
        let [body_area, buttons_area] =
            *Layout::vertical([Constraint::Length(4), Constraint::Max(3)]).split(popup_area)
        else {
            return Ok(());
        };

        let input = Input::default()
            .set_label(self.input_label)
            .set_label_style(app.config.as_text_style())
            .set_text(&self.value)
            .set_focused(self.input_focused)
            .set_focused_style(app.config.theme.highlight_border_style)
            .set_unfocused_style(app.config.as_border_style());

        self.button_group.set_active_style(if self.input_focused {
            Style::default().reversed()
        } else {
            app.config.theme.current_item_style
        });

        self.input_area = body_area;

        frame.render_widget(input, block.inner(body_area));
        frame.render_widget(block, body_area);
        frame.render_stateful_widget(&mut self.button_group, buttons_area, &mut self.button_group_state);
        Ok(())
    }

    fn handle_key(&mut self, key: &mut KeyEvent, context: &mut AppContext) -> Result<()> {
        let action = key.as_common_action(context);
        if self.input_focused {
            if let Some(CommonAction::Close) = action {
                self.input_focused = false;

                context.render()?;
                return Ok(());
            } else if let Some(CommonAction::Confirm) = action {
                if self.button_group_state.selected == 0 {
                    if let Some(ref mut callback) = self.callback {
                        (callback)(context, &self.value)?;
                    }
                }
                pop_modal!(context);
                return Ok(());
            }

            match key.code() {
                KeyCode::Char(c) => {
                    self.value.push(c);

                    context.render()?;
                }
                KeyCode::Backspace => {
                    self.value.pop();

                    context.render()?;
                }
                _ => {}
            }
        } else if let Some(action) = action {
            match action {
                CommonAction::Down => {
                    self.button_group_state.next();

                    context.render()?;
                }
                CommonAction::Up => {
                    self.button_group_state.next();

                    context.render()?;
                }
                CommonAction::Close => {
                    pop_modal!(context);
                }
                CommonAction::Confirm => {
                    if self.button_group_state.selected == 0 {
                        if let Some(ref mut callback) = self.callback {
                            (callback)(context, &self.value)?;
                        }
                    }
                    pop_modal!(context);
                }
                CommonAction::FocusInput => {
                    self.input_focused = true;

                    context.render()?;
                }
                _ => {}
            }
        };

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &mut AppContext) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick => {
                if let Some(idx) = self.button_group.get_button_idx_at(event.into()) {
                    self.button_group_state.select(idx);
                    self.input_focused = false;
                    context.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                match self.button_group.get_button_idx_at(event.into()) {
                    Some(0) => {
                        if let Some(ref mut callback) = self.callback {
                            (callback)(context, &self.value)?;
                        }
                        pop_modal!(context);
                    }
                    Some(_) => {
                        pop_modal!(context);
                    }
                    None => {
                        if self.input_area.contains(event.into()) {
                            self.input_focused = true;
                            context.render()?;
                        }
                    }
                };
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollUp => {
                if self.button_group.get_button_idx_at(event.into()).is_some() {
                    self.input_focused = false;
                    self.button_group_state.prev();
                    context.render()?;
                }
            }
            MouseEventKind::ScrollDown => {
                if self.button_group.get_button_idx_at(event.into()).is_some() {
                    self.input_focused = false;
                    self.button_group_state.next();
                    context.render()?;
                }
            }
        }
        Ok(())
    }
}
