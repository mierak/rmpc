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
    shared::{key_event::KeyEvent, macros::pop_modal},
    ui::widgets::button::{Button, ButtonGroup, ButtonGroupState},
};

use super::RectExt;

use super::Modal;

const BUTTON_GROUP_SYMBOLS: symbols::border::Set = symbols::border::Set {
    top_right: symbols::line::NORMAL.vertical_left,
    top_left: symbols::line::NORMAL.vertical_right,
    ..symbols::border::ROUNDED
};

#[derive(Default, Debug)]
pub struct ConfirmQueueClearModal {
    button_group: ButtonGroupState,
}

impl Modal for ConfirmQueueClearModal {
    fn render(&mut self, frame: &mut Frame, app: &mut AppContext) -> Result<()> {
        let block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_set(border::ROUNDED)
            .border_style(app.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center)
            .title("Clear the queue?");
        let text = Paragraph::new("Are you sure you want to clear the queue?")
            .style(app.config.as_text_style())
            .wrap(Wrap { trim: true });

        let popup_area = frame.area().centered_exact(45, 5);
        frame.render_widget(Clear, popup_area);

        if let Some(bg_color) = app.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }
        let [text_area, buttons_area] =
            *Layout::vertical([Constraint::Length(2), Constraint::Max(3)]).split(popup_area)
        else {
            return Ok(());
        };

        let buttons = vec![Button::default().label("Clear"), Button::default().label("Cancel")];
        self.button_group.set_button_count(buttons.len());
        let group = ButtonGroup::default()
            .active_style(app.config.theme.current_item_style)
            .inactive_style(app.config.as_text_style())
            .buttons(buttons)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(BUTTON_GROUP_SYMBOLS)
                    .border_style(app.config.as_border_style()),
            );

        frame.render_widget(
            text,
            block.inner(popup_area).inner(Margin {
                horizontal: 1,
                vertical: 0,
            }),
        );
        frame.render_widget(block, text_area);
        frame.render_stateful_widget(group, buttons_area, &mut self.button_group);
        Ok(())
    }

    fn handle_key(&mut self, key: &mut KeyEvent, client: &mut Client<'_>, context: &mut AppContext) -> Result<()> {
        if let Some(action) = key.as_common_action(context) {
            match action {
                CommonAction::Down => {
                    self.button_group.next();

                    context.render()?;
                }
                CommonAction::Up => {
                    self.button_group.prev();

                    context.render()?;
                }
                CommonAction::Close => {
                    self.button_group = ButtonGroupState::default();
                    pop_modal!(context);
                }
                CommonAction::Confirm => {
                    if self.button_group.selected == 0 {
                        client.clear()?;
                    }
                    self.button_group = ButtonGroupState::default();
                    pop_modal!(context);
                }
                _ => {}
            }
        };

        Ok(())
    }
}
