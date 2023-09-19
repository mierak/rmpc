use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::{Backend, Constraint, Direction, Layout, Margin},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::{
    mpd::{client::Client, mpd_client::MpdClient},
    state::State,
    ui::widgets::button::{Button, ButtonGroup, ButtonGroupState},
};

use super::{KeyHandleResultInternal, RectExt, SharedUiState};

use super::Modal;

#[derive(Debug, Default)]
pub struct ConfirmQueueClearModal {
    button_group: ButtonGroupState,
}

#[async_trait]
impl Modal for ConfirmQueueClearModal {
    fn render<B: Backend>(
        &mut self,
        frame: &mut Frame<B>,
        _app: &mut crate::state::State,
        _shared_state: &mut SharedUiState,
    ) -> Result<()> {
        let block = Block::default().borders(Borders::ALL).title("Clear the queue?");
        let text = Paragraph::new("Are you sure you want to clear the queue?").wrap(Wrap { trim: true });

        let popup_area = frame.size().centered_exact(20, 7);
        let [text_area, buttons_area] = *Layout::default()
            .constraints([Constraint::Length(3), Constraint::Max(1)].as_ref())
            .direction(Direction::Vertical)
            .split(block.inner(popup_area.inner(&Margin {horizontal: 1, vertical: 0}))) else { return Ok(()); };

        let group = ButtonGroup::default().buttons(vec![
            Button::default().label("Clear"),
            Button::default().label("Cancel"),
        ]);

        frame.render_widget(Clear, popup_area);
        frame.render_widget(block, popup_area);
        frame.render_widget(text, text_area);
        frame.render_stateful_widget(group, buttons_area, &mut self.button_group);
        Ok(())
    }

    async fn handle_key(
        &mut self,
        key: KeyEvent,
        _client: &mut Client<'_>,
        _app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<KeyHandleResultInternal> {
        match key.code {
            KeyCode::Char('j') => {
                if self.button_group.selected == 1 {
                    self.button_group.selected = 0;
                } else {
                    self.button_group.selected += 1;
                }
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            KeyCode::Char('k') => {
                if self.button_group.selected == 0 {
                    self.button_group.selected = 1;
                } else {
                    self.button_group.selected -= 1;
                }
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            KeyCode::Esc => {
                // _shared.visible_modal = None;
                self.button_group = ButtonGroupState::default();
                Ok(KeyHandleResultInternal::Modal(None))
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // _shared.visible_modal = None;
                self.button_group = ButtonGroupState::default();
                Ok(KeyHandleResultInternal::Modal(None))
            }
            KeyCode::Enter => {
                if self.button_group.selected == 0 {
                    _client.clear().await?;
                }
                // _shared.visible_modal = None;
                self.button_group = ButtonGroupState::default();
                Ok(KeyHandleResultInternal::Modal(None))
            }
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }
}
