use anyhow::{Context, Result, anyhow};
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::tabs::PaneType,
    context::AppContext,
    mpd::{
        QueuePosition,
        client::Client,
        commands::{Song, lsinfo::LsInfoEntry},
        mpd_client::{Filter, MpdClient, SingleOrRange, Tag},
    },
    shared::{
        ext::mpd_client::MpdClientExt,
        key_event::KeyEvent,
        macros::{modal, status_error, status_info},
        mouse_event::MouseEvent,
        mpd_query::PreviewGroup,
    },
    ui::{
        UiEvent,
        browser::{AddCommand, BrowserPane, MoveDirection},
        dir_or_song::DirOrSong,
        dirstack::{DirStack, DirStackItem},
        modals::{
            confirm_modal::ConfirmModal,
            info_list_modal::InfoListModal,
            input_modal::InputModal,
        },
        widgets::browser::Browser,
    },
};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct PlaylistsPane {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
    browser: Browser<DirOrSong>,
    initialized: bool,
    selected_song: Option<(usize, String)>,
}

const INIT: &str = "init";
const REINIT: &str = "reinit";
const OPEN_OR_PLAY: &str = "open_or_play";
const PREVIEW: &str = "preview";
const PLAYLIST_INFO: &str = "preview";

impl PlaylistsPane {
    pub fn new(_context: &AppContext) -> Self {
        Self {
            stack: DirStack::default(),
            filter_input_mode: false,
            browser: Browser::new(),
            initialized: false,
            selected_song: None,
        }
    }

    fn open_or_play(
        &mut self,
        autoplay: bool,
        context: &AppContext,
        action_id: &'static str,
    ) -> Result<()> {
        let Some(selected) = self.stack().current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");

            context.render()?;
            return Ok(());
        };
        let Some(next_path) = self.stack.next_path() else {
            log::error!("Failed to move deeper inside dir. Next path is None");
            return Ok(());
        };

        match selected {
            DirOrSong::Dir { name: playlist, .. } => {
                let playlist = playlist.clone();
                context.query().id(action_id).target(PaneType::Playlists).query(move |client| {
                    Ok(MpdQueryResult::SongsList {
                        data: client.list_playlist_info(&playlist, None)?,
                        origin_path: Some(next_path),
                    })
                });
                self.stack_mut().push(Vec::new());
                self.stack_mut().clear_preview();
                context.render()?;
            }
            DirOrSong::Song(_song) => {
                if let Some(add) = self.add(selected, context) {
                    let queue_len = context.queue.len();
                    context.command(move |client| {
                        add(client, None)?;
                        if autoplay {
                            client.play_last(queue_len)?;
                        }
                        Ok(())
                    });
                }
            }
        }

        Ok(())
    }
}

impl Pane for PlaylistsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> Result<()> {
        self.browser.set_filter_input_active(self.filter_input_mode).render(
            area,
            frame.buffer_mut(),
            &mut self.stack,
            &context.config,
        );

        Ok(())
    }

    fn before_show(&mut self, context: &AppContext) -> Result<()> {
        if !self.initialized {
            context.query().id(INIT).target(PaneType::Playlists).replace_id(INIT).query(
                move |client| {
                    let result: Vec<_> = client
                        .list_playlists()
                        .context("Cannot list playlists")?
                        .into_iter()
                        .sorted_by(|a, b| a.name.cmp(&b.name))
                        .map(|playlist| DirOrSong::name_only(playlist.name))
                        .collect();
                    Ok(MpdQueryResult::DirOrSong { data: result, origin_path: None })
                },
            );

            self.initialized = true;
        }
        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut UiEvent,
        _is_visible: bool,
        context: &AppContext,
    ) -> Result<()> {
        let id = match event {
            UiEvent::Database => Some(INIT),
            UiEvent::StoredPlaylist => Some(REINIT),
            _ => None,
        };
        match event {
            UiEvent::Database | UiEvent::StoredPlaylist => {
                if let Some(id) = id {
                    context.query().id(id).replace_id(id).target(PaneType::Playlists).query(
                        move |client| {
                            let result: Vec<_> = client
                                .list_playlists()
                                .context("Cannot list playlists")?
                                .into_iter()
                                .sorted_by(|a, b| a.name.cmp(&b.name))
                                .map(|playlist| DirOrSong::name_only(playlist.name))
                                .collect();
                            Ok(MpdQueryResult::DirOrSong { data: result, origin_path: None })
                        },
                    );
                }
            }
            UiEvent::Reconnected => {
                self.initialized = false;
                self.before_show(context)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &AppContext) -> Result<()> {
        self.handle_mouse_action(event, context)
    }

    fn handle_action(&mut self, event: &mut KeyEvent, context: &mut AppContext) -> Result<()> {
        self.handle_filter_input(event, context)?;
        self.handle_common_action(event, context)?;
        self.handle_global_action(event, context)?;
        Ok(())
    }

    fn on_query_finished(
        &mut self,
        id: &'static str,
        mpd_command: MpdQueryResult,
        _is_visible: bool,
        context: &AppContext,
    ) -> Result<()> {
        match (id, mpd_command) {
            (PLAYLIST_INFO, MpdQueryResult::SongsList { data, .. }) => {
                modal!(
                    context,
                    InfoListModal::builder()
                        .column_widths(&[30, 70])
                        .title("Playlist info")
                        .items(data)
                        .size((40, 20))
                        .build()
                );
                context.render()?;
            }
            (PREVIEW, MpdQueryResult::Preview { data, origin_path }) => {
                if let Some(origin_path) = origin_path {
                    if origin_path != self.stack().path() {
                        log::trace!(origin_path:?, current_path:? = self.stack().path(); "Dropping preview because it does not belong to this path");
                        return Ok(());
                    }
                }
                self.stack_mut().set_preview(data);
                context.render()?;
            }
            (OPEN_OR_PLAY, MpdQueryResult::SongsList { data, origin_path }) => {
                if let Some(origin_path) = origin_path {
                    if origin_path != self.stack().path() {
                        log::trace!(origin_path:?, current_path:? = self.stack().path(); "Dropping result because it does not belong to this path");
                        return Ok(());
                    }
                }
                self.stack_mut().replace(data.into_iter().map(DirOrSong::Song).collect());
                self.prepare_preview(context)?;
                context.render()?;
            }
            (INIT, MpdQueryResult::DirOrSong { data, origin_path: _ }) => {
                self.stack = DirStack::new(data);
                self.prepare_preview(context)?;
            }
            (REINIT, MpdQueryResult::DirOrSong { data, .. }) => {
                let mut new_stack = DirStack::new(data);
                let old_viewport_len = self.stack.current().state.viewport_len();
                let old_content_len = self.stack.current().state.content_len();
                match self.stack.path() {
                    [playlist_name] => {
                        let (selected_idx, selected_playlist) = self
                            .stack()
                            .previous()
                            .selected_with_idx()
                            .map_or((0, playlist_name.as_str()), |(idx, playlist)| {
                                (idx, playlist.as_path())
                            });
                        let idx_to_select = new_stack
                            .current()
                            .items
                            .iter()
                            .find_position(|item| item.as_path() == selected_playlist)
                            .map_or(selected_idx, |(idx, _)| idx);
                        new_stack.current_mut().state.set_viewport_len(old_viewport_len);

                        new_stack
                            .current_mut()
                            .state
                            .select(Some(idx_to_select), context.config.scrolloff);

                        if let Some((idx, DirOrSong::Song(song))) =
                            self.stack().current().selected_with_idx()
                        {
                            self.selected_song = Some((idx, song.as_path().to_owned()));
                        }
                        let playlist = playlist_name.to_owned();
                        self.stack = new_stack;
                        self.stack_mut().current_mut().state.set_content_len(old_content_len);
                        self.stack_mut().current_mut().state.set_viewport_len(old_viewport_len);

                        let songs = context.query_sync(move |client| {
                            Ok(client.list_playlist_info(&playlist, None)?)
                        })?;

                        self.stack_mut().push(songs.into_iter().map(DirOrSong::Song).collect());
                        self.prepare_preview(context)?;
                        if let Some((idx, song)) = &self.selected_song {
                            let idx_to_select = self
                                .stack
                                .current()
                                .items
                                .iter()
                                .find_position(|item| item.as_path() == song)
                                .map_or(*idx, |(idx, _)| idx);
                            self.stack.current_mut().state.set_viewport_len(old_viewport_len);
                            self.stack
                                .current_mut()
                                .state
                                .select(Some(idx_to_select), context.config.scrolloff);
                        }
                        self.stack_mut().clear_preview();
                        self.prepare_preview(context)?;
                        context.render()?;
                    }
                    [] => {
                        let Some((selected_idx, selected_playlist)) = self
                            .stack()
                            .current()
                            .selected_with_idx()
                            .map(|(idx, playlist)| (idx, playlist.as_path()))
                        else {
                            log::warn!(stack:? = self.stack(); "Expected playlist to be selected");
                            return Ok(());
                        };
                        let idx_to_select = new_stack
                            .current()
                            .items
                            .iter()
                            .find_position(|item| item.as_path() == selected_playlist)
                            .map_or(selected_idx, |(idx, _)| idx);
                        new_stack.current_mut().state.set_viewport_len(old_viewport_len);
                        new_stack
                            .current_mut()
                            .state
                            .select(Some(idx_to_select), context.config.scrolloff);

                        self.stack = new_stack;
                        self.prepare_preview(context)?;
                    }
                    _ => {
                        log::error!(stack:? = self.stack; "Invalid playlist stack state");
                    }
                }
            }
            _ => {}
        }
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

    fn show_info(&self, item: &DirOrSong, context: &AppContext) -> Result<()> {
        match item {
            DirOrSong::Dir { name, .. } => {
                let playlist = name.clone();
                context
                    .query()
                    .target(PaneType::Playlists)
                    .replace_id(PLAYLIST_INFO)
                    .id(PLAYLIST_INFO)
                    .query(move |client| {
                        let playlist = client.list_playlist_info(&playlist, None)?;
                        Ok(MpdQueryResult::SongsList { data: playlist, origin_path: None })
                    });
            }
            DirOrSong::Song(_) => {}
        }
        Ok(())
    }

    fn list_songs_in_item(
        &self,
        item: DirOrSong,
    ) -> impl FnOnce(&mut Client<'_>) -> Result<Vec<Song>> + 'static {
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
                    ConfirmModal::builder()
                    .context(context)
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
                        .size((45, 6))
                        .build()
                );
            }
            DirOrSong::Song(s) => {
                let Some(DirOrSong::Dir { name: playlist, .. }) = self.stack.previous().selected()
                else {
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
        }
        Ok(())
    }

    fn add_all(&self, context: &AppContext, position: Option<QueuePosition>) -> Result<()> {
        match self.stack().path() {
            [playlist] => {
                let playlist = playlist.clone();
                context.command(move |client| {
                    client.load_playlist(&playlist, position)?;
                    status_info!("Playlist '{playlist}' added to queue");
                    Ok(())
                });
            }
            [] => {
                for playlist in self.stack().current().items.iter().rev() {
                    if let Some(add) = self.add(playlist, context) {
                        context.command(move |client| {
                            add(client, position)?;
                            // status_info!("Playlist '{}' added to queue", playlist.as_path());
                            Ok(())
                        });
                    }
                }
                status_info!("All playlists added to queue");
            }
            _ => {}
        }

        Ok(())
    }

    fn add(&self, item: &DirOrSong, context: &AppContext) -> Option<Box<dyn AddCommand>> {
        match item {
            DirOrSong::Dir { name: d, .. } => {
                let d = d.clone();
                Some(Box::new(move |client, position| {
                    client.load_playlist(&d, position)?;
                    status_info!("Playlist '{d}' added to queue");
                    Ok(())
                }))
            }
            DirOrSong::Song(s) => {
                let file = s.file.clone();
                let artist_text =
                    s.artist_str(&context.config.theme.format_tag_separator).into_owned();
                let title_text =
                    s.title_str(&context.config.theme.format_tag_separator).into_owned();

                Some(Box::new(move |client, position| {
                    client.add(&file, position)?;
                    if let Ok(Some(_song)) = client.find_one(&[Filter::new(Tag::File, &file)]) {
                        status_info!("'{}' by '{}' added to queue", title_text, artist_text);
                    }
                    Ok(())
                }))
            }
        }
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
                                    status_info!(
                                        "Playlist '{}' renamed to '{}'",
                                        current_name,
                                        new_value
                                    );
                                    Ok(())
                                });
                            }
                            Ok(())
                        })
                );
            }
            DirOrSong::Song(_) => {}
        }

        Ok(())
    }

    fn open(&mut self, context: &AppContext) -> Result<()> {
        self.open_or_play(true, context, OPEN_OR_PLAY)
    }

    fn next(&mut self, context: &AppContext) -> Result<()> {
        self.open_or_play(false, context, OPEN_OR_PLAY)
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
                self.stack_mut().current_mut().items.swap(idx, new_idx);
                self.stack_mut().current_mut().select_idx(new_idx, context.config.scrolloff);
            }
        }
        context.render()?;

        Ok(())
    }

    fn prepare_preview(&mut self, context: &AppContext) -> Result<()> {
        let config = std::sync::Arc::clone(&context.config);
        let s = self.stack().current().selected().cloned();
        self.stack_mut().clear_preview();
        let origin_path = Some(self.stack().path().to_vec());
        context
            .query()
            .id(PREVIEW)
            .replace_id("playlists_preview")
            .target(PaneType::Playlists)
            .query(move |client| {
                let data = s.as_ref().map_or(Ok(None), move |current| -> Result<_> {
                    let response = match current {
                        DirOrSong::Dir { name: d, .. } => Some(vec![PreviewGroup::from(
                            None,
                            None,
                            client
                                .list_playlist_info(d, None)?
                                .into_iter()
                                .map(DirOrSong::Song)
                                .map(|s| s.to_list_item_simple(&config))
                                .collect_vec(),
                        )]),
                        DirOrSong::Song(song) => {
                            match client
                                .lsinfo(Some(&song.file))
                                .context(anyhow!("File '{}' was listed but not found", song.file))?
                                .0
                                .first()
                            {
                                Some(LsInfoEntry::File(song)) => Some(song.to_preview(
                                    config.theme.preview_label_style,
                                    config.theme.preview_metadata_group_style,
                                )),
                                _ => None,
                            }
                        }
                    };
                    Ok(response)
                })?;

                Ok(MpdQueryResult::Preview { data, origin_path })
            });
        Ok(())
    }

    fn browser_areas(&self) -> [Rect; 3] {
        self.browser.areas
    }
}
