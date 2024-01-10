use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::{
    prelude::{Constraint, Direction, Layout, Rect},
    Frame,
};

use crate::{mpd::client::Client, state::State};

use self::{
    confirm_queue_clear::ConfirmQueueClearModal, rename_playlist::RenamePlaylistModal, save_queue::SaveQueueModal,
};

use super::{KeyHandleResultInternal, SharedUiState};

pub mod confirm_queue_clear;
pub mod rename_playlist;
pub mod save_queue;

#[derive(Debug)]
pub enum Modals {
    ConfirmQueueClear(ConfirmQueueClearModal),
    SaveQueue(SaveQueueModal),
    RenamePlaylist(RenamePlaylistModal),
}

pub(super) trait Modal {
    fn render(
        &mut self,
        frame: &mut Frame,
        _app: &mut crate::state::State,
        _shared_state: &mut SharedUiState,
    ) -> Result<()>;

    // todo global modal keys (esc, ctrl c)
    fn handle_key(
        &mut self,
        key: KeyEvent,
        _client: &mut Client<'_>,
        _app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<KeyHandleResultInternal>;
}

pub trait RectExt {
    fn centered(&self, width_percent: u16, height_percent: u16) -> Rect;
    fn centered_exact(&self, width: u16, height: u16) -> Rect;
}

impl RectExt for Rect {
    fn centered(&self, width_percent: u16, height_percent: u16) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage((100 - height_percent) / 2),
                    Constraint::Percentage(height_percent),
                    Constraint::Percentage((100 - height_percent) / 2),
                ]
                .as_ref(),
            )
            .split(*self);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage((100 - width_percent) / 2),
                    Constraint::Percentage(width_percent),
                    Constraint::Percentage((100 - width_percent) / 2),
                ]
                .as_ref(),
            )
            .split(popup_layout[1])[1]
    }

    fn centered_exact(&self, width: u16, height: u16) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage((100 - height) / 2),
                    Constraint::Min(height),
                    Constraint::Percentage((100 - height) / 2),
                ]
                .as_ref(),
            )
            .split(*self);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage((100 - width) / 2),
                    Constraint::Min(width),
                    Constraint::Percentage((100 - width) / 2),
                ]
                .as_ref(),
            )
            .split(popup_layout[1])[1]
    }
}
