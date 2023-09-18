use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::{Backend, Constraint, Direction, Layout, Margin},
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

use super::{KeyHandleResult, RectExt, SharedUiState};

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

#[async_trait]
impl Modal for SaveQueueModal {
    fn render<B: Backend>(
        &mut self,
        frame: &mut Frame<B>,
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
        let [text_area,input_area, buttons_area] = *Layout::default()
            .constraints([Constraint::Length(1), Constraint::Length(3), Constraint::Max(1)].as_ref())
            .direction(Direction::Vertical)
            .split(block.inner(popup_area.inner(&Margin {horizontal: 1, vertical: 0}))) else { return Ok(()); };

        let group =
            ButtonGroup::default().buttons(vec![Button::default().label("Save"), Button::default().label("Cancel")]);

        frame.render_widget(Clear, popup_area);
        frame.render_widget(block, popup_area);
        frame.render_widget(text, text_area);
        frame.render_widget(input, input_area);
        frame.render_stateful_widget(group, buttons_area, &mut self.button_group);
        Ok(())
    }

    async fn handle_key(
        &mut self,
        key: KeyEvent,
        _client: &mut Client<'_>,
        _app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<KeyHandleResult> {
        if self.input_focused {
            return match key.code {
                KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {
                    self.input_focused = false;
                    Ok(KeyHandleResult::RenderRequested)
                }
                KeyCode::Char(c) => {
                    self.name.push(c);
                    Ok(KeyHandleResult::RenderRequested)
                }
                KeyCode::Backspace => {
                    self.name.pop();
                    Ok(KeyHandleResult::RenderRequested)
                }
                KeyCode::Enter => {
                    if self.button_group.selected == 0 {
                        _client.save_queue_as_playlist(&self.name, None).await?;
                        _shared.status_message = Some(StatusMessage::new(
                            format!("Playlist '{}' saved", self.name),
                            Level::Info,
                        ));
                    }
                    _app.visible_modal = None;
                    self.on_hide();
                    Ok(KeyHandleResult::RenderRequested)
                }
                KeyCode::Esc => {
                    self.input_focused = false;
                    Ok(KeyHandleResult::RenderRequested)
                }
                _ => Ok(KeyHandleResult::SkipRender),
            };
        }
        match key.code {
            KeyCode::Char('i') => {
                self.input_focused = true;
            }
            KeyCode::Char('j') => {
                if self.button_group.selected == 1 {
                    self.button_group.selected = 0;
                } else {
                    self.button_group.selected += 1;
                }
            }
            KeyCode::Char('k') => {
                if self.button_group.selected == 0 {
                    self.button_group.selected = 1;
                } else {
                    self.button_group.selected -= 1;
                }
            }
            KeyCode::Esc => {
                _app.visible_modal = None;
                self.on_hide();
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                _app.visible_modal = None;
                self.on_hide();
            }
            KeyCode::Enter => {
                if self.button_group.selected == 0 {
                    _client.save_queue_as_playlist(&self.name, None).await?;
                    _shared.status_message = Some(StatusMessage::new(
                        format!("Playlist '{}' saved", self.name),
                        Level::Info,
                    ));
                }
                _app.visible_modal = None;
                self.on_hide();
            }
            _ => {}
        }
        Ok(KeyHandleResult::RenderRequested)
    }
}
