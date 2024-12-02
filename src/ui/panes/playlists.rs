use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use ratatui::{prelude::Rect, widgets::StatefulWidget, Frame};

use crate::{
    config::tabs::PaneType,
    context::AppContext,
    mpd::{
        client::Client,
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
        modals::{confirm_modal::ConfirmModal, input_modal::InputModal},
        widgets::browser::Browser,
        UiEvent,
    },
    MpdQueryResult,
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

    fn open_or_play(&mut self, autoplay: bool, context: &AppContext) -> Result<()> {
        let Some(selected) = self.stack().current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");

            context.render()?;
            return Ok(());
        };

        match selected {
            DirOrSong::Dir { name: playlist, .. } => {
                let playlist = playlist.clone();
                context.query("open_or_play", PaneType::Playlists, move |client| {
                    Ok(MpdQueryResult::SongsList(client.list_playlist_info(&playlist, None)?))
                });
                self.stack_mut().push(Vec::new());
                self.stack_mut().clear_preview();
                context.render()?;
            }
            DirOrSong::Song(_song) => {
                self.add(selected, context)?;
                let queue_len = context.queue.len();
                if autoplay {
                    context.command(move |client| Ok(client.play_last(queue_len)?));
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

    fn before_show(&mut self, context: &AppContext) -> Result<()> {
        if !self.initialized {
            context.query("init", PaneType::Playlists, move |client| {
                let result: Vec<_> = client
                    .list_playlists()
                    .context("Cannot list playlists")?
                    .into_iter()
                    .map(|playlist| DirOrSong::Dir {
                        name: playlist.name,
                        full_path: String::new(),
                    })
                    .sorted()
                    .collect();
                Ok(MpdQueryResult::DirOrSong(result))
            });

            self.initialized = true;
        }
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, context: &AppContext) -> Result<()> {
        let id = match event {
            UiEvent::Database => Some("init"),
            UiEvent::StoredPlaylist => Some("list_playlists"),
            _ => None,
        };

        if let Some(id) = id {
            context.query(id, PaneType::Playlists, move |client| {
                let result: Vec<_> = client
                    .list_playlists()
                    .context("Cannot list playlists")?
                    .into_iter()
                    .map(|playlist| DirOrSong::Dir {
                        name: playlist.name,
                        full_path: String::new(),
                    })
                    .sorted()
                    .collect();
                Ok(MpdQueryResult::DirOrSong(result))
            });
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &AppContext) -> Result<()> {
        self.handle_mouse_action(event, context)
    }

    fn handle_action(&mut self, event: &mut KeyEvent, context: &AppContext) -> Result<()> {
        self.handle_filter_input(event, context)?;
        self.handle_common_action(event, context)?;
        self.handle_global_action(event, context)?;
        Ok(())
    }

    fn on_query_finished(&mut self, id: &'static str, mpd_command: MpdQueryResult, context: &AppContext) -> Result<()> {
        match mpd_command {
            MpdQueryResult::Preview(vec) => {
                self.stack_mut().set_preview(vec);
                context.render()?;
            }
            MpdQueryResult::SongsList(data) => {
                self.stack_mut()
                    .replace(data.into_iter().map(DirOrSong::Song).collect());
                self.prepare_preview(context);
                context.render()?;
            }
            MpdQueryResult::DirOrSong(data) => {
                if id == "init" {
                    self.stack = DirStack::new(data);
                } else {
                    let mut new_stack = DirStack::new(data);
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
                            self.next(context)?;

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
                }
                self.prepare_preview(context);
                context.render()?;
            }
            _ => {}
        };
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

    fn list_songs_in_item(&self, item: DirOrSong) -> impl FnOnce(&mut Client<'_>) -> Result<Vec<Song>> + 'static {
        move |client| {
            Ok(match item {
                DirOrSong::Dir { name, .. } => client.list_playlist_info(&name, None)?,
                DirOrSong::Song(song) => vec![song.clone()],
            })
        }
    }

    fn delete(&self, item: &DirOrSong, index: usize, context: &AppContext) -> Result<()> {
        match item {
            DirOrSong::Dir { name: d, .. } => {
                let d = d.clone();
                modal!(
                    context,
                    ConfirmModal::new(context)
                        .message("Are you sure you want to delete this playlist? This action cannot be undone.")
                        .on_confirm(move |context| {
                            let d = d.clone();
                            context.command(move |client| {
                                client.delete_playlist(&d)?;
                                status_info!("Playlist '{d}' deleted");
                                Ok(())
                            });
                            Ok(())
                        })
                        .confirm_label("Delete")
                        .size(45, 6)
                );
            }
            DirOrSong::Song(s) => {
                let Some(DirOrSong::Dir { name: playlist, .. }) = self.stack.previous().selected() else {
                    return Ok(());
                };
                let playlist = playlist.clone();
                let file = s.file.clone();
                context.command(move |client| {
                    client.delete_from_playlist(&playlist, &SingleOrRange::single(index))?;
                    status_info!("File '{file}' deleted from playlist '{playlist}'");
                    Ok(())
                });

                context.render()?;
            }
        };
        Ok(())
    }

    fn add_all(&self, context: &AppContext) -> Result<()> {
        match self.stack().path() {
            [playlist] => {
                let playlist = playlist.clone();
                context.command(move |client| {
                    client.load_playlist(&playlist)?;
                    status_info!("Playlist '{playlist}' added to queue");
                    Ok(())
                });
            }
            [] => {
                for playlist in &self.stack().current().items {
                    self.add(playlist, context)?;
                }
                status_info!("All playlists added to queue");
            }
            _ => {}
        };

        Ok(())
    }

    fn add(&self, item: &DirOrSong, context: &AppContext) -> Result<()> {
        match item {
            DirOrSong::Dir { name: d, .. } => {
                let d = d.clone();
                context.command(move |client| {
                    client.load_playlist(&d)?;
                    status_info!("Playlist '{d}' added to queue");
                    Ok(())
                });
            }
            DirOrSong::Song(s) => {
                let file = s.file.clone();
                context.command(move |client| {
                    client.add(&file)?;
                    if let Ok(Some(song)) = client.find_one(&[Filter::new(Tag::File, &file)]) {
                        status_info!("'{}' by '{}' added to queue", song.title_str(), song.artist_str());
                    }
                    Ok(())
                });
            }
        };

        Ok(())
    }

    fn rename(&self, item: &DirOrSong, context: &AppContext) -> Result<()> {
        match item {
            DirOrSong::Dir { name: d, .. } => {
                let current_name = d.clone();
                modal!(
                    context,
                    InputModal::new(context)
                        .title("Rename playlist")
                        .confirm_label("Rename")
                        .input_label("New name:")
                        .initial_value(current_name.clone())
                        .on_confirm(move |context, new_value| {
                            if current_name != new_value {
                                let current_name = current_name.clone();
                                let new_value = new_value.to_owned();
                                context.command(move |client| {
                                    client.rename_playlist(&current_name, &new_value)?;
                                    status_info!("Playlist '{}' renamed to '{}'", current_name, new_value);
                                    Ok(())
                                });
                            }
                            Ok(())
                        })
                );
            }
            DirOrSong::Song(_) => {}
        };

        Ok(())
    }

    fn open(&mut self, context: &AppContext) -> Result<()> {
        self.open_or_play(true, context)
    }

    fn next(&mut self, context: &AppContext) -> Result<()> {
        self.open_or_play(false, context)
    }

    fn move_selected(&mut self, direction: MoveDirection, context: &AppContext) -> Result<()> {
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
                let playlist = playlist.clone();
                context.command(move |client| {
                    client.move_in_playlist(&playlist, &SingleOrRange::single(idx), new_idx)?;
                    Ok(())
                });
            }
        };

        Ok(())
    }

    fn prepare_preview(&self, context: &AppContext) {
        let config = context.config;
        let s = self.stack().current().selected().cloned();
        context.query("preview", PaneType::Playlists, move |c| {
            let result = s.as_ref().map_or(Ok(None), move |current| -> Result<_> {
                Ok(Some(match current {
                    DirOrSong::Dir { name: d, .. } => c
                        .list_playlist_info(d, None)?
                        .into_iter()
                        .map(DirOrSong::Song)
                        .map(|s| s.to_list_item_simple(config))
                        .collect_vec(),
                    DirOrSong::Song(song) => c
                        .find_one(&[Filter::new(Tag::File, &song.file)])?
                        .context(anyhow!("File '{}' was listed but not found", song.file))?
                        .to_preview(&config.theme.symbols)
                        .collect_vec(),
                }))
            })?;

            Ok(MpdQueryResult::Preview(result))
        });
    }

    fn browser_areas(&self) -> [Rect; 3] {
        self.browser.areas
    }
}
