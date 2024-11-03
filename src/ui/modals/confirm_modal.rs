use anyhow::Result;
use ratatui::{
    prelude::{Constraint, Layout, Margin},
    style::Style,
    symbols::{self, border},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::{
    config::keys::CommonAction,
    context::AppContext,
    mpd::{client::Client, mpd_client::MpdClient},
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

#[derive(Debug)]
pub struct ConfirmModal<'a> {
    title: String,
    message: String,
    button_group_state: ButtonGroupState,
    button_group: ButtonGroup<'a>,
}

#[allow(dead_code)]
impl ConfirmModal<'_> {
    pub fn new(title: String, message: String, context: &AppContext) -> Self {
        let mut button_group_state = ButtonGroupState::default();
        let buttons = vec![Button::default().label("Save"), Button::default().label("Cancel")];
        button_group_state.set_button_count(buttons.len());

        let button_group = ButtonGroup::default()
            .active_style(context.config.theme.current_item_style)
            .inactive_style(context.config.as_text_style())
            .add_button(Button::default().label("Ok"))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(BUTTON_GROUP_SYMBOLS)
                    .border_style(context.config.as_border_style()),
            );

        Self {
            title,
            message,
            button_group_state,
            button_group,
        }
    }
}

impl Modal for ConfirmModal<'_> {
    fn render(&mut self, frame: &mut Frame, app: &mut AppContext) -> Result<()> {
        let block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_set(border::ROUNDED)
            .border_style(app.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center)
            .title(self.title.as_str());
        let text = Paragraph::new(self.message.as_str())
            .style(app.config.as_text_style())
            .wrap(Wrap { trim: true });

        let popup_area = frame.area().centered_exact(45, 7);
        frame.render_widget(Clear, popup_area);

        if let Some(bg_color) = app.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }
        let [text_area, buttons_area] =
            *Layout::vertical([Constraint::Length(4), Constraint::Max(3)]).split(popup_area)
        else {
            return Ok(());
        };

        frame.render_widget(
            text,
            block.inner(popup_area).inner(Margin {
                horizontal: 1,
                vertical: 0,
            }),
        );
        frame.render_widget(block, text_area);
        frame.render_stateful_widget(&mut self.button_group, buttons_area, &mut self.button_group_state);
        Ok(())
    }

    fn handle_key(&mut self, key: &mut KeyEvent, client: &mut Client<'_>, context: &mut AppContext) -> Result<()> {
        if let Some(action) = key.as_common_action(context) {
            match action {
                CommonAction::Down => {
                    self.button_group_state.next();

                    context.render()?;
                }
                CommonAction::Up => {
                    self.button_group_state.prev();

                    context.render()?;
                }
                CommonAction::Close => {
                    self.button_group_state = ButtonGroupState::default();
                    pop_modal!(context);
                }
                CommonAction::Confirm => {
                    if self.button_group_state.selected == 0 {
                        client.clear()?;
                    }
                    self.button_group_state = ButtonGroupState::default();
                    pop_modal!(context);
                }
                _ => {}
            }
        };

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
                        client.clear()?;
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
