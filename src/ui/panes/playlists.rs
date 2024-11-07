use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use ratatui::{
    prelude::Rect,
    widgets::{ListItem, StatefulWidget},
    Frame,
};

use crate::{
    config::Config,
    context::AppContext,
    mpd::{
        commands::Song,
        mpd_client::{Filter, MpdClient, SingleOrRange, Tag},
    },
    shared::{
        ext::mpd_client::MpdClientExt,
        key_event::KeyEvent,
        macros::{modal, status_error, status_info},
        mouse_event::MouseEvent,
    },
    ui::{
        browser::{BrowserPane, MoveDirection},
        dirstack::{DirStack, DirStackItem},
        modals::rename_playlist::RenamePlaylistModal,
        widgets::browser::Browser,
        UiEvent,
    },
};

use super::{browser::DirOrSong, Pane};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct PlaylistsPane {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
    browser: Browser<DirOrSong>,
    initialized: bool,
}

impl PlaylistsPane {
    pub fn new(context: &AppContext) -> Self {
        Self {
            stack: DirStack::default(),
            filter_input_mode: false,
            browser: Browser::new(context.config),
            initialized: false,
        }
    }

    fn open_or_play(&mut self, autoplay: bool, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        let Some(selected) = self.stack().current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");

            context.render()?;
            return Ok(());
        };

        match selected {
            DirOrSong::Dir { name: playlist, .. } => {
                let info = client.list_playlist_info(playlist, None)?;
                self.stack_mut().push(info.into_iter().map(DirOrSong::Song).collect());

                context.render()?;
            }
            DirOrSong::Song(_song) => {
                self.add(selected, client, context)?;
                if autoplay {
                    client.play_last(context)?;
                }
            }
        };

        Ok(())
    }
}

impl Pane for PlaylistsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, _context: &AppContext) -> Result<()> {
        self.browser
            .set_filter_input_active(self.filter_input_mode)
            .render(area, frame.buffer_mut(), &mut self.stack);

        Ok(())
    }

    fn before_show(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        if !self.initialized {
            let playlists: Vec<_> = client
                .list_playlists()
                .context("Cannot list playlists")?
                .into_iter()
                .map(|playlist| DirOrSong::Dir {
                    name: playlist.name,
                    full_path: String::new(),
                })
                .sorted()
                .collect();
            self.stack = DirStack::new(playlists);
            let preview = self
                .prepare_preview(client, context.config)
                .context("Cannot prepare preview")?;
            self.stack.set_preview(preview);
            self.initialized = true;
        }
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        match event {
            UiEvent::Database => {
                let playlists: Vec<_> = client
                    .list_playlists()
                    .context("Cannot list playlists")?
                    .into_iter()
                    .map(|playlist| DirOrSong::Dir {
                        name: playlist.name,
                        full_path: String::new(),
                    })
                    .sorted()
                    .collect();
                self.stack = DirStack::new(playlists);
                let preview = self
                    .prepare_preview(client, context.config)
                    .context("Cannot prepare preview")?;
                self.stack.set_preview(preview);
                context.render()?;
            }
            UiEvent::StoredPlaylist => {
                let mut new_stack = DirStack::new(
                    client
                        .list_playlists()
                        .context("Cannot list playlists")?
                        .into_iter()
                        .map(|playlist| DirOrSong::Dir {
                            name: playlist.name,
                            full_path: String::new(),
                        })
                        .sorted()
                        .collect_vec(),
                );
                let old_viewport_len = self.stack.current().state.viewport_len();

                match self.stack.current_mut().selected_mut() {
                    Some(DirOrSong::Dir { name: playlist, .. }) => {
                        let mut items = new_stack.current().items.iter();
                        // Select the same playlist by name or index as before
                        let idx_to_select = items
                            .find_position(|p| matches!(p, DirOrSong::Dir { name: d, .. } if d == playlist))
                            .or_else(|| self.stack().current().selected_with_idx())
                            .map(|(idx, _)| idx);

                        new_stack.current_mut().state.set_viewport_len(old_viewport_len);
                        new_stack
                            .current_mut()
                            .state
                            .select(idx_to_select, context.config.scrolloff);

                        self.stack = new_stack;
                    }
                    Some(DirOrSong::Song(ref mut song)) => {
                        let song = std::mem::take(song);
                        let playlist = &self.stack.path()[0];
                        let mut items = new_stack.current().items.iter();
                        // Select the same playlist by name or index as before
                        let playlist_idx_to_select = items
                            .find_position(|p| matches!(p, DirOrSong::Dir { name: d, .. } if d == playlist))
                            .or_else(|| self.stack().previous().selected_with_idx())
                            .map(|(idx, _)| idx);

                        new_stack.current_mut().state.set_viewport_len(old_viewport_len);
                        new_stack
                            .current_mut()
                            .state
                            .select(playlist_idx_to_select, context.config.scrolloff);

                        let previous_song_index = self.stack.current().selected_with_idx().map(|(idx, _)| idx);
                        self.stack = new_stack;
                        self.next(client, context)?;

                        // Select the same song by filename or index as before
                        let mut items = self.stack.current().items.iter();
                        let idx_to_select = items
                            .find_position(|p| matches!(p, DirOrSong::Song(s) if s.file == song.file))
                            .map(|(idx, _)| idx)
                            .or(previous_song_index);
                        self.stack.current_mut().state.set_viewport_len(old_viewport_len);
                        self.stack
                            .current_mut()
                            .state
                            .select(idx_to_select, context.config.scrolloff);
                    }
                    None => {}
                }

                let preview = self
                    .prepare_preview(client, context.config)
                    .context("Cannot prepare preview")?;
                self.stack.set_preview(preview);

                context.render()?;
            }
            _ => {}
        };

        Ok(())
    }

    fn handle_mouse_event(
        &mut self,
        event: MouseEvent,
        client: &mut impl MpdClient,
        context: &mut AppContext,
    ) -> Result<()> {
        self.handle_mouse_action(event, client, context)
    }

    fn handle_action(&mut self, event: &mut KeyEvent, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        self.handle_filter_input(event, client, context)?;
        self.handle_common_action(event, client, context)?;
        self.handle_global_action(event, client, context)?;
        Ok(())
    }
}

impl BrowserPane<DirOrSong> for PlaylistsPane {
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

    fn list_songs_in_item(&self, client: &mut impl MpdClient, item: &DirOrSong) -> Result<Vec<Song>> {
        Ok(match item {
            DirOrSong::Dir { name, .. } => client.list_playlist_info(name, None)?,
            DirOrSong::Song(song) => vec![song.clone()],
        })
    }

    fn delete(&self, item: &DirOrSong, index: usize, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        match item {
            DirOrSong::Dir { name: d, .. } => {
                client.delete_playlist(d)?;
                status_info!("Playlist '{d}' deleted");

                context.render()?;
            }
            DirOrSong::Song(s) => {
                let Some(DirOrSong::Dir { name: playlist, .. }) = self.stack.previous().selected() else {
                    return Ok(());
                };
                client.delete_from_playlist(playlist, &SingleOrRange::single(index))?;
                status_info!("File '{}' deleted from playlist '{playlist}'", s.file);

                context.render()?;
            }
        };
        Ok(())
    }

    fn add_all(&self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        match self.stack().path() {
            [playlist] => {
                client.load_playlist(playlist)?;
                status_info!("Playlist '{playlist}' added to queue");

                context.render()?;
            }
            [] => {
                for playlist in &self.stack().current().items {
                    self.add(playlist, client, context)?;
                }
                status_info!("All playlists added to queue");

                context.render()?;
            }
            _ => {}
        };

        Ok(())
    }

    fn add(&self, item: &DirOrSong, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        match item {
            DirOrSong::Dir { name: d, .. } => {
                client.load_playlist(d)?;
                status_info!("Playlist '{d}' added to queue");

                context.render()?;
            }
            DirOrSong::Song(s) => {
                client.add(&s.file)?;
                if let Ok(Some(song)) = client.find_one(&[Filter::new(Tag::File, &s.file)]) {
                    status_info!("'{}' by '{}' added to queue", song.title_str(), song.artist_str());
                }

                context.render()?;
            }
        };

        Ok(())
    }

    fn rename(&self, item: &DirOrSong, _client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        match item {
            DirOrSong::Dir { name: d, .. } => {
                modal!(context, RenamePlaylistModal::new(d.clone(), context));
            }
            DirOrSong::Song(_) => {}
        };

        Ok(())
    }

    fn open(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        self.open_or_play(true, client, context)
    }

    fn next(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        self.open_or_play(false, client, context)
    }

    fn move_selected(&mut self, direction: MoveDirection, client: &mut impl MpdClient) -> Result<()> {
        let Some((idx, selected)) = self.stack().current().selected_with_idx() else {
            status_error!("Failed to move playlist. No playlist selected");
            return Ok(());
        };
        let Some(DirOrSong::Dir { name: playlist, .. }) = self.stack.previous().selected() else {
            return Ok(());
        };

        match selected {
            DirOrSong::Dir { .. } => {}
            DirOrSong::Song(_) => {
                let new_idx = match direction {
                    MoveDirection::Up => idx.saturating_sub(1),
                    MoveDirection::Down => (idx + 1).min(self.stack().current().items.len() - 1),
                };
                client.move_in_playlist(playlist, &SingleOrRange::single(idx), new_idx)?;
            }
        };

        Ok(())
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
                    DirOrSong::Dir { name: d, .. } => client
                        .list_playlist_info(d, None)?
                        .into_iter()
                        .map(DirOrSong::Song)
                        .map(|s| s.to_list_item_simple(config))
                        .collect_vec(),
                    DirOrSong::Song(song) => client
                        .find_one(&[Filter::new(Tag::File, &song.file)])?
                        .context(anyhow!("File '{}' was listed but not found", song.file))?
                        .to_preview(&config.theme.symbols)
                        .collect_vec(),
                }))
            })
    }

    fn browser_areas(&self) -> [Rect; 3] {
        self.browser.areas
    }
}
