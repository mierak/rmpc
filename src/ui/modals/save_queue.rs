use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
        _app: &mut crate::state::State,
        _shared_state: &mut SharedUiState,
    ) -> Result<()> {
        let block = Block::default().borders(Borders::ALL).title("Save queue as playlist");
        let text = Paragraph::new("Playlist name:").wrap(Wrap { trim: true });
        let input = Paragraph::new(self.name.clone())
            .block(Block::default().borders(Borders::ALL).fg(if self.input_focused {
                Color::Blue
            } else {
                Color::White
            }))
            .fg(Color::White)
            .wrap(Wrap { trim: true });

        let popup_area = frame.size().centered_exact(20, 7);
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

        frame.render_widget(Clear, popup_area);
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
        _app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<KeyHandleResultInternal> {
        if self.input_focused {
            return match key.code {
                KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {
                    self.input_focused = false;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Char(c) => {
                    self.name.push(c);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Backspace => {
                    self.name.pop();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Enter => {
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
                KeyCode::Esc => {
                    self.input_focused = false;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                _ => Ok(KeyHandleResultInternal::SkipRender),
            };
        }
        match key.code {
            KeyCode::Char('i') => {
                self.input_focused = true;
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            KeyCode::Char('j') => {
                self.button_group.next();
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            KeyCode::Char('k') => {
                self.button_group.prev();
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            KeyCode::Esc => Ok(KeyHandleResultInternal::Modal(None)),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.on_hide();
                Ok(KeyHandleResultInternal::Modal(None))
            }
            KeyCode::Enter => {
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
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }
}
