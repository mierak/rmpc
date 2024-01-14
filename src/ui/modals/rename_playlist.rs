use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::{Constraint, Direction, Layout, Margin},
    style::{Color, Stylize},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::{
    mpd::{client::Client, mpd_client::MpdClient},
    state::State,
    ui::{
        screens::CommonAction,
        widgets::button::{Button, ButtonGroup, ButtonGroupState},
        Level, StatusMessage,
    },
};

use super::{KeyHandleResultInternal, RectExt, SharedUiState};

use super::Modal;

#[derive(Debug)]
pub struct RenamePlaylistModal {
    button_group: ButtonGroupState,
    input_focused: bool,
    playlist_name: String,
    new_name: String,
}

impl Default for RenamePlaylistModal {
    fn default() -> Self {
        Self {
            button_group: ButtonGroupState::default(),
            input_focused: true,
            playlist_name: String::new(),
            new_name: String::new(),
        }
    }
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
    fn render(
        &mut self,
        frame: &mut Frame,
        _app: &mut crate::state::State,
        _shared_state: &mut SharedUiState,
    ) -> Result<()> {
        let block = Block::default().borders(Borders::ALL).title("Rename playlist");
        let input = Paragraph::new(self.new_name.clone())
            .block(Block::default().borders(Borders::ALL).fg(if self.input_focused {
                Color::Blue
            } else {
                Color::White
            }))
            .fg(Color::White)
            .wrap(Wrap { trim: true });

        let popup_area = frame.size().centered_exact(20, 6);
        let [input_area, buttons_area] = *Layout::default()
            .constraints([Constraint::Length(3), Constraint::Max(1)].as_ref())
            .direction(Direction::Vertical)
            .split(block.inner(popup_area.inner(&Margin {
                horizontal: 1,
                vertical: 0,
            })))
        else {
            return Ok(());
        };

        let buttons = vec![Button::default().label("Save"), Button::default().label("Cancel")];
        self.button_group.set_button_count(buttons.len());
        let group = ButtonGroup::default().buttons(buttons);

        frame.render_widget(Clear, popup_area);
        frame.render_widget(block, popup_area);
        frame.render_widget(input, input_area);
        frame.render_stateful_widget(group, buttons_area, &mut self.button_group);
        Ok(())
    }

    fn handle_key(
        &mut self,
        key: KeyEvent,
        client: &mut Client<'_>,
        app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<KeyHandleResultInternal> {
        let action = app.config.keybinds.navigation.get(&key.into());
        if self.input_focused {
            if let Some(CommonAction::Close) = action {
                self.input_focused = false;
                return Ok(KeyHandleResultInternal::RenderRequested);
            } else if let Some(CommonAction::Confirm) = action {
                if self.button_group.selected == 0 && self.playlist_name != self.new_name {
                    client.rename_playlist(&self.playlist_name, &self.new_name)?;
                    _shared.status_message = Some(StatusMessage::new(
                        format!("Playlist '{}' renamed te '{}'", self.playlist_name, self.new_name),
                        Level::Info,
                    ));
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
                        _shared.status_message = Some(StatusMessage::new(
                            format!("Playlist '{}' renamed te '{}'", self.playlist_name, self.new_name),
                            Level::Info,
                        ));
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
                CommonAction::Delete => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Rename => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else {
            Ok(KeyHandleResultInternal::SkipRender)
        }
    }
}
