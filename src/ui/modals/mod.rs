use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::{
    prelude::{Constraint, Layout, Rect},
    Frame,
};

use crate::{mpd::client::Client, state::State};

use self::{
    add_to_playlist::AddToPlaylistModal, confirm_queue_clear::ConfirmQueueClearModal,
    rename_playlist::RenamePlaylistModal, save_queue::SaveQueueModal,
};

use super::{KeyHandleResultInternal, SharedUiState};

pub mod add_to_playlist;
pub mod confirm_queue_clear;
pub mod rename_playlist;
pub mod save_queue;

#[derive(Debug)]
pub enum Modals {
    ConfirmQueueClear(ConfirmQueueClearModal),
    SaveQueue(SaveQueueModal),
    RenamePlaylist(RenamePlaylistModal),
    AddToPlaylist(AddToPlaylistModal),
}

pub(super) trait Modal {
    fn render(
        &mut self,
        frame: &mut Frame,
        _app: &mut crate::state::State,
        _shared_state: &mut SharedUiState,
    ) -> Result<()>;

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
        let popup_layout = Layout::vertical([
            Constraint::Percentage((100 - height_percent) / 2),
            Constraint::Percentage(height_percent),
            Constraint::Percentage((100 - height_percent) / 2),
        ])
        .split(*self);

        Layout::horizontal([
            Constraint::Percentage((100 - width_percent) / 2),
            Constraint::Percentage(width_percent),
            Constraint::Percentage((100 - width_percent) / 2),
        ])
        .split(popup_layout[1])[1]
    }

    fn centered_exact(&self, width: u16, height: u16) -> Rect {
        let popup_layout = Layout::vertical([
            Constraint::Length((self.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Length((self.height.saturating_sub(height)) / 2),
        ])
        .split(*self);

        Layout::horizontal([
            Constraint::Length((self.width.saturating_sub(width)) / 2),
            Constraint::Length(width),
            Constraint::Length((self.width.saturating_sub(width)) / 2),
        ])
        .split(popup_layout[1])[1]
    }
}

#[cfg(test)]
mod tests {
    use ratatui::prelude::Rect;

    use super::RectExt;

    #[test]
    fn exact() {
        let input = Rect {
            x: 25,
            y: 25,
            width: 250,
            height: 250,
        };

        let result = input.centered_exact(60, 50);

        assert_eq!(
            result,
            Rect {
                x: 120,
                y: 125,
                width: 60,
                height: 50,
            }
        );
    }

    #[test]
    fn exact_width_exceeded_gives_max_possible_size() {
        let input = Rect {
            x: 25,
            y: 25,
            width: 10,
            height: 10,
        };

        let result = input.centered_exact(60, 50);

        assert_eq!(result, input);
    }
}
