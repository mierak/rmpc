use anyhow::Result;
use ratatui::{
    prelude::{Constraint, Layout},
    style::Style,
    symbols::{self, border},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::{
    config::keys::{CommonAction, GlobalAction},
    context::AppContext,
    mpd::client::Client,
    shared::{
        key_event::KeyEvent,
        macros::pop_modal,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::widgets::button::{Button, ButtonGroup, ButtonGroupState},
};

use super::RectExt;

use super::Modal;

const BUTTON_GROUP_SYMBOLS: symbols::border::Set = symbols::border::Set {
    top_right: symbols::line::NORMAL.vertical_left,
    top_left: symbols::line::NORMAL.vertical_right,
    ..symbols::border::ROUNDED
};

pub struct ConfirmModal<'a, Callback: FnMut(&mut Client<'_>) -> Result<()> + 'a> {
    message: &'a str,
    button_group_state: ButtonGroupState,
    button_group: ButtonGroup<'a>,
    callback: Option<Callback>,
    size: (u16, u16),
}

impl<'a, Callback: FnMut(&mut Client<'_>) -> Result<()> + 'a> std::fmt::Debug for ConfirmModal<'_, Callback> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ConfirmModal(message = {}, button_group = {:?}, button_group_state = {:?})",
            self.message, self.button_group, self.button_group_state,
        )
    }
}

#[allow(dead_code)]
impl<'a, Callback: FnMut(&mut Client<'_>) -> Result<()> + 'a> ConfirmModal<'a, Callback> {
    pub fn new(context: &AppContext) -> Self {
        let mut button_group_state = ButtonGroupState::default();
        let buttons = vec![Button::default().label("Confirm"), Button::default().label("Cancel")];
        button_group_state.set_button_count(buttons.len());
        let button_group = ButtonGroup::default()
            .active_style(context.config.theme.current_item_style)
            .inactive_style(context.config.as_text_style())
            .buttons(buttons)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(BUTTON_GROUP_SYMBOLS)
                    .border_style(context.config.as_border_style()),
            );

        Self {
            message: "",
            button_group_state,
            button_group,
            callback: None,
            size: (45, 6),
        }
    }

    pub fn size(mut self, cols: u16, rows: u16) -> Self {
        self.size = (cols, rows);
        self
    }

    pub fn confirm_label(mut self, label: &'a str) -> Self {
        let buttons = vec![Button::default().label(label), Button::default().label("Cancel")];
        self.button_group = self.button_group.buttons(buttons);
        self
    }

    pub fn on_confirm(mut self, callback: Callback) -> Self {
        self.callback = Some(callback);
        self
    }

    pub fn message(mut self, message: &'a str) -> Self {
        self.message = message;
        self
    }
}

impl<'a, Callback: FnMut(&mut Client<'_>) -> Result<()> + 'a> Modal for ConfirmModal<'_, Callback> {
    fn render(&mut self, frame: &mut Frame, app: &mut AppContext) -> Result<()> {
        let popup_area = frame.area().centered_exact(self.size.0, self.size.1);
        frame.render_widget(Clear, popup_area);

        if let Some(bg_color) = app.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_set(border::ROUNDED)
            .border_style(app.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center);

        let paragraph = Paragraph::new(self.message)
            .style(app.config.as_text_style())
            .wrap(Wrap { trim: true })
            .block(block.clone())
            .centered();

        let [content_area, buttons_area] =
            *Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).split(popup_area)
        else {
            return Ok(());
        };

        frame.render_widget(paragraph, content_area);
        frame.render_stateful_widget(&mut self.button_group, buttons_area, &mut self.button_group_state);
        Ok(())
    }

    fn handle_key(&mut self, key: &mut KeyEvent, client: &mut Client<'_>, context: &mut AppContext) -> Result<()> {
        if let Some(action) = key.as_common_action(context) {
            match action {
                CommonAction::Right => {
                    self.button_group_state.next();
                    context.render()?;
                }
                CommonAction::Left => {
                    self.button_group_state.prev();
                    context.render()?;
                }
                CommonAction::Close => {
                    self.button_group_state = ButtonGroupState::default();
                    pop_modal!(context);
                }
                CommonAction::Confirm => {
                    if self.button_group_state.selected == 0 {
                        if let Some(ref mut callback) = self.callback {
                            (callback)(client)?;
                        }
                    }
                    self.button_group_state = ButtonGroupState::default();
                    pop_modal!(context);
                }
                _ => {}
            }
        } else if let Some(action) = key.as_global_action(context) {
            match action {
                GlobalAction::NextTab => {
                    self.button_group_state.next();
                    context.render()?;
                }
                GlobalAction::PreviousTab => {
                    self.button_group_state.prev();
                    context.render()?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn handle_mouse_event(
        &mut self,
        event: MouseEvent,
        client: &mut Client<'_>,
        context: &mut AppContext,
    ) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick => {
                if let Some(idx) = self.button_group.get_button_idx_at(event.into()) {
                    self.button_group_state.select(idx);
                    context.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                match self.button_group.get_button_idx_at(event.into()) {
                    Some(0) => {
                        if let Some(ref mut callback) = self.callback {
                            (callback)(client)?;
                        }
                        pop_modal!(context);
                    }
                    Some(_) => {
                        pop_modal!(context);
                    }
                    None => {}
                };
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollUp => {
                if self.button_group.get_button_idx_at(event.into()).is_some() {
                    self.button_group_state.prev();
                    context.render()?;
                }
            }
            MouseEventKind::ScrollDown => {
                if self.button_group.get_button_idx_at(event.into()).is_some() {
                    self.button_group_state.next();
                    context.render()?;
                }
            }
        }
        Ok(())
    }
}
