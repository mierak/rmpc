use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::{Constraint, Layout},
    style::{Style, Stylize},
    symbols::{self, border},
    widgets::{Block, Borders, Clear},
    Frame,
};

use crate::{
    config::keys::CommonAction,
    mpd::{client::Client, mpd_client::MpdClient},
    state::State,
    ui::widgets::{
        button::{Button, ButtonGroup, ButtonGroupState},
        input::Input,
    },
    utils::macros::status_info,
};

use super::{KeyHandleResultInternal, RectExt};

use super::Modal;

const BUTTON_GROUP_SYMBOLS: symbols::border::Set = symbols::border::Set {
    top_right: symbols::line::NORMAL.vertical_left,
    top_left: symbols::line::NORMAL.vertical_right,
    ..symbols::border::ROUNDED
};

#[derive(Debug)]
pub struct RenamePlaylistModal {
    button_group: ButtonGroupState,
    input_focused: bool,
    playlist_name: String,
    new_name: String,
}

impl RenamePlaylistModal {
    pub fn new(playlist_name: String) -> Self {
        Self {
            new_name: playlist_name.clone(),
            playlist_name,
            button_group: ButtonGroupState::default(),
            input_focused: true,
        }
    }
    fn on_hide(&mut self) {
        self.button_group = ButtonGroupState::default();
        self.playlist_name = String::new();
        self.input_focused = true;
    }
}

impl Modal for RenamePlaylistModal {
    fn render(&mut self, frame: &mut Frame, app: &mut crate::state::State) -> Result<()> {
        let block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_set(border::ROUNDED)
            .border_style(app.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center)
            .title("Rename playlist");

        let popup_area = frame.size().centered_exact(50, 7);
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
            .set_label("New name:")
            .set_label_style(app.config.as_text_style())
            .set_text(&self.new_name)
            .set_focused(self.input_focused)
            .set_focused_style(app.config.theme.highlight_border_style)
            .set_unfocused_style(app.config.as_border_style());

        let buttons = vec![Button::default().label("Save"), Button::default().label("Cancel")];
        self.button_group.set_button_count(buttons.len());
        let group = ButtonGroup::default()
            .buttons(buttons)
            .inactive_style(app.config.as_text_style())
            .active_style(if self.input_focused {
                Style::default().reversed()
            } else {
                app.config.theme.current_item_style
            })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_set(BUTTON_GROUP_SYMBOLS)
                    .border_style(app.config.as_border_style()),
            );

        frame.render_widget(input, block.inner(body_area));
        frame.render_widget(block, body_area);
        frame.render_stateful_widget(group, buttons_area, &mut self.button_group);
        Ok(())
    }

    fn handle_key(
        &mut self,
        key: KeyEvent,
        client: &mut Client<'_>,
        app: &mut State,
    ) -> Result<KeyHandleResultInternal> {
        let action = app.config.keybinds.navigation.get(&key.into());
        if self.input_focused {
            if let Some(CommonAction::Close) = action {
                self.input_focused = false;
                return Ok(KeyHandleResultInternal::RenderRequested);
            } else if let Some(CommonAction::Confirm) = action {
                if self.button_group.selected == 0 && self.playlist_name != self.new_name {
                    client.rename_playlist(&self.playlist_name, &self.new_name)?;
                    status_info!("Playlist '{}' renamed to '{}'", self.playlist_name, self.new_name);
                }
                self.on_hide();
                return Ok(KeyHandleResultInternal::Modal(None));
            }

            match key.code {
                KeyCode::Char(c) => {
                    self.new_name.push(c);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Backspace => {
                    self.new_name.pop();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                _ => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else if let Some(action) = action {
            match action {
                CommonAction::Down => {
                    self.button_group.next();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Up => {
                    self.button_group.next();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Close => {
                    self.on_hide();
                    Ok(KeyHandleResultInternal::Modal(None))
                }
                CommonAction::Confirm => {
                    if self.button_group.selected == 0 && self.playlist_name != self.new_name {
                        client.rename_playlist(&self.playlist_name, &self.new_name)?;
                        status_info!("Playlist '{}' renamed to '{}'", self.playlist_name, self.new_name);
                    }
                    self.on_hide();
                    Ok(KeyHandleResultInternal::Modal(None))
                }
                CommonAction::FocusInput => {
                    self.input_focused = true;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::MoveDown => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::MoveUp => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::DownHalf => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::UpHalf => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Right => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Left => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Top => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Bottom => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::EnterSearch => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::NextResult => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::PreviousResult => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Select => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Add => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::AddAll => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Delete => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Rename => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else {
            Ok(KeyHandleResultInternal::SkipRender)
        }
    }
}
