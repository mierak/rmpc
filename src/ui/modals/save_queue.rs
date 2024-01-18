use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::{Constraint, Direction, Layout, Margin},
    style::{Color, Style, Stylize},
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
pub struct SaveQueueModal {
    button_group: ButtonGroupState,
    input_focused: bool,
    name: String,
}

impl Default for SaveQueueModal {
    fn default() -> Self {
        Self {
            button_group: ButtonGroupState::default(),
            input_focused: true,
            name: String::new(),
        }
    }
}

impl SaveQueueModal {
    fn on_hide(&mut self) {
        self.button_group = ButtonGroupState::default();
        self.name = String::new();
        self.input_focused = true;
    }
}

impl Modal for SaveQueueModal {
    fn render(
        &mut self,
        frame: &mut Frame,
        app: &mut crate::state::State,
        _shared_state: &mut SharedUiState,
    ) -> Result<()> {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(app.config.as_border_style())
            .title("Save queue as playlist");
        let text = Paragraph::new("Playlist name:").wrap(Wrap { trim: true });
        let input = Paragraph::new(self.name.clone())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(if self.input_focused {
                        app.config.ui.highlight_border_style
                    } else {
                        app.config.as_border_style()
                    }),
            )
            .fg(Color::White)
            .wrap(Wrap { trim: true });

        let popup_area = frame.size().centered_exact(20, 7);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = app.config.ui.background_color_modal {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }
        let [text_area, input_area, buttons_area] = *Layout::default()
            .constraints([Constraint::Length(1), Constraint::Length(3), Constraint::Max(1)].as_ref())
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

        frame.render_widget(block, popup_area);
        frame.render_widget(text, text_area);
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
                if self.button_group.selected == 0 {
                    client.save_queue_as_playlist(&self.name, None)?;
                    _shared.status_message = Some(StatusMessage::new(
                        format!("Playlist '{}' saved", self.name),
                        Level::Info,
                    ));
                }
                self.on_hide();
                return Ok(KeyHandleResultInternal::Modal(None));
            }

            match key.code {
                KeyCode::Char(c) => {
                    self.name.push(c);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Backspace => {
                    self.name.pop();
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
                    if self.button_group.selected == 0 {
                        client.save_queue_as_playlist(&self.name, None)?;
                        _shared.status_message = Some(StatusMessage::new(
                            format!("Playlist '{}' saved", self.name),
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
