use anyhow::{anyhow, Context, Result};
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::{prelude::Rect, widgets::ListItem, Frame};
use strum::Display;

use crate::{
    config::Config,
    mpd::{
        commands::Status,
        mpd_client::{Filter, MpdClient, SingleOrRange, Tag},
    },
    ui::{
        modals::{rename_playlist::RenamePlaylistModal, Modals},
        utils::dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        KeyHandleResultInternal, UiEvent,
    },
    utils::macros::{status_error, status_info},
};

use super::{browser::DirOrSong, BrowserScreen, Screen};

#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
pub struct PlaylistsScreen {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum PlaylistsActions {}

impl Screen for PlaylistsScreen {
    type Actions = PlaylistsActions;
    fn render(&mut self, frame: &mut Frame, area: Rect, _status: &Status, config: &Config) -> Result<()> {
        frame.render_stateful_widget(
            Browser::new(config)
                .set_widths(&config.ui.column_widths)
                .set_border_style(config.as_border_style()),
            area,
            &mut self.stack,
        );

        Ok(())
    }

    fn before_show(&mut self, client: &mut impl MpdClient, _status: &mut Status, config: &Config) -> Result<()> {
        if self.stack().path().is_empty() {
            let mut playlists: Vec<_> = client
                .list_playlists()
                .context("Cannot list playlists")?
                .into_iter()
                .map(|playlist| DirOrSong::Dir(playlist.name))
                .collect();
            playlists.sort();
            self.stack = DirStack::new(playlists);
            let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
            self.stack.set_preview(preview);
        }
        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut UiEvent,
        client: &mut impl MpdClient,
        _status: &mut Status,
        config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        match event {
            UiEvent::StoredPlaylist | UiEvent::Database => {
                let mut new_stack = DirStack::new(
                    client
                        .list_playlists()
                        .context("Cannot list playlists")?
                        .into_iter()
                        .map(|playlist| DirOrSong::Dir(playlist.name))
                        .sorted()
                        .collect_vec(),
                );

                match self.stack.current_mut().selected_mut() {
                    Some(DirOrSong::Dir(playlist)) => {
                        let mut items = new_stack.current().items.iter();
                        // Select the same playlist by name or index as before
                        let idx_to_select = items
                            .find_position(|p| matches!(p, DirOrSong::Dir(d) if d == playlist))
                            .or_else(|| self.stack().current().selected_with_idx())
                            .map(|(idx, _)| idx);
                        new_stack.current_mut().state.select(idx_to_select);

                        self.stack = new_stack;
                    }
                    Some(DirOrSong::Song(ref mut song)) => {
                        let song = std::mem::take(song);
                        let playlist = &self.stack.path()[0];
                        let mut items = new_stack.current().items.iter();
                        // Select the same playlist by name or index as before
                        let playlist_idx_to_select = items
                            .find_position(|p| matches!(p, DirOrSong::Dir(d) if d == playlist))
                            .or_else(|| self.stack().previous().selected_with_idx())
                            .map(|(idx, _)| idx);
                        new_stack.current_mut().state.select(playlist_idx_to_select);

                        let previous_song_index = self.stack.current().selected_with_idx().map(|(idx, _)| idx);
                        self.stack = new_stack;
                        self.next(client)?;

                        // Select the same song by filename or index as before
                        let mut items = self.stack.current().items.iter();
                        let idx_to_select = items
                            .find_position(|p| matches!(p, DirOrSong::Song(s) if s == &song))
                            .map(|(idx, _)| idx)
                            .or(previous_song_index);
                        self.stack.current_mut().state.select(idx_to_select);
                    }
                    None => {}
                }

                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack.set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }

    fn handle_action(
        &mut self,
        event: KeyEvent,
        client: &mut impl MpdClient,
        _status: &mut Status,
        config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        if self.filter_input_mode {
            self.handle_filter_input(event, client, config)
        } else if let Some(_action) = config.keybinds.playlists.get(&event.into()) {
            Ok(KeyHandleResultInternal::SkipRender)
        } else if let Some(action) = config.keybinds.navigation.get(&event.into()) {
            self.handle_common_action(*action, client, config)
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

impl BrowserScreen<DirOrSong> for PlaylistsScreen {
    fn stack(&self) -> &DirStack<DirOrSong> {
        &self.stack
    }

    fn stack_mut(&mut self) -> &mut DirStack<DirOrSong> {
        &mut self.stack
    }

    fn set_filter_input_mode_active(&mut self, active: bool) {
        self.filter_input_mode = active;
    }

    fn is_filter_input_mode_active(&self) -> bool {
        self.filter_input_mode
    }

    fn delete(&self, item: &DirOrSong, index: usize, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        match item {
            DirOrSong::Dir(d) => {
                client.delete_playlist(d)?;
                status_info!("Playlist '{d}' deleted");
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            DirOrSong::Song(s) => {
                let Some(DirOrSong::Dir(playlist)) = self.stack.previous().selected() else {
                    return Ok(KeyHandleResultInternal::SkipRender);
                };
                client.delete_from_playlist(playlist, &SingleOrRange::single(index))?;
                status_info!("File '{s}' deleted from playlist '{playlist}'");
                Ok(KeyHandleResultInternal::RenderRequested)
            }
        }
    }

    fn add(&self, item: &DirOrSong, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        match item {
            DirOrSong::Dir(d) => {
                client.load_playlist(d)?;
                status_info!("Playlist '{d}' added to queue");
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            DirOrSong::Song(s) => {
                client.add(s)?;
                if let Ok(Some(song)) = client.find_one(&[Filter::new(Tag::File, s)]) {
                    status_info!("'{}' by '{}' added to queue", song.title_str(), song.artist_str());
                }
                Ok(KeyHandleResultInternal::RenderRequested)
            }
        }
    }

    fn rename(&self, item: &DirOrSong, _client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        match item {
            DirOrSong::Dir(d) => Ok(KeyHandleResultInternal::Modal(Some(Modals::RenamePlaylist(
                RenamePlaylistModal::new(d.clone()),
            )))),
            DirOrSong::Song(_) => Ok(KeyHandleResultInternal::SkipRender),
        }
    }

    fn next(&mut self, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        let Some(selected) = self.stack().current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(KeyHandleResultInternal::RenderRequested);
        };

        match selected {
            DirOrSong::Dir(playlist) => {
                let info = client.list_playlist(playlist)?;
                self.stack_mut().push(info.into_iter().map(DirOrSong::Song).collect());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            DirOrSong::Song(_song) => self.add(selected, client),
        }
    }

    fn move_selected(
        &mut self,
        direction: super::MoveDirection,
        client: &mut impl MpdClient,
    ) -> Result<KeyHandleResultInternal> {
        let Some((idx, selected)) = self.stack().current().selected_with_idx() else {
            status_error!("Failed to move playlist. No playlist selected");
            return Ok(KeyHandleResultInternal::SkipRender);
        };
        let Some(DirOrSong::Dir(playlist)) = self.stack.previous().selected() else {
            return Ok(KeyHandleResultInternal::SkipRender);
        };

        match selected {
            DirOrSong::Dir(_) => {}
            DirOrSong::Song(_) => {
                let new_idx = match direction {
                    super::MoveDirection::Up => idx.saturating_sub(1),
                    super::MoveDirection::Down => (idx + 1).min(self.stack().current().items.len() - 1),
                };
                client.move_in_playlist(playlist, &SingleOrRange::single(idx), new_idx)?;
            }
        }
        Ok(KeyHandleResultInternal::SkipRender)
    }

    fn prepare_preview(
        &mut self,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<Option<Vec<ListItem<'static>>>> {
        self.stack()
            .current()
            .selected()
            .map_or(Ok(None), |current| -> Result<_> {
                Ok(Some(match current {
                    DirOrSong::Dir(d) => client
                        .list_playlist(d)?
                        .into_iter()
                        .map(DirOrSong::Song)
                        .map(|s| s.to_list_item(config, false, None))
                        .collect_vec(),
                    DirOrSong::Song(file) => client
                        .find_one(&[Filter::new(Tag::File, file)])?
                        .context(anyhow!("File '{file}' was listed but not found"))?
                        .to_preview(&config.ui.symbols)
                        .collect_vec(),
                }))
            })
    }
}
