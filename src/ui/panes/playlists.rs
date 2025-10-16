use std::{collections::BTreeSet, sync::Arc};

use anyhow::{Context, Result};
use enum_map::EnumMap;
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect, widgets::ListState};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::{keys::actions::Position, tabs::PaneType},
    ctx::Ctx,
    mpd::{
        client::Client,
        commands::Song,
        mpd_client::{MpdClient, SingleOrRange},
    },
    shared::{
        cmp::StringCompare,
        ext::btreeset_ranges::BTreeSetRanges,
        key_event::KeyEvent,
        macros::{modal, status_info},
        mouse_event::MouseEvent,
        mpd_client_ext::{Autoplay, MpdClientExt, MpdDelete},
    },
    status_warn,
    ui::{
        UiEvent,
        browser::{BrowserPane, MoveDirection},
        dir_or_song::DirOrSong,
        dirstack::{DirStack, DirStackItem},
        modals::{info_list_modal::InfoListModal, input_modal::InputModal},
        widgets::browser::{Browser, BrowserArea},
    },
};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct PlaylistsPane {
    stack: DirStack<DirOrSong, ListState>,
    filter_input_mode: bool,
    browser: Browser<DirOrSong>,
    initialized: bool,
}

const INIT: &str = "init";
const REINIT: &str = "reinit";
const FETCH_DATA: &str = "fetch_data";
const PLAYLIST_INFO: &str = "preview";

impl PlaylistsPane {
    pub fn new(_ctx: &Ctx) -> Self {
        Self {
            stack: DirStack::default(),
            filter_input_mode: false,
            browser: Browser::new(),
            initialized: false,
        }
    }

    fn open_or_play(&mut self, autoplay: bool, ctx: &Ctx) -> Result<()> {
        let Some(selected) = self.stack().current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };

        match selected {
            DirOrSong::Dir { .. } => {
                self.stack_mut().enter();
                ctx.render()?;
            }
            DirOrSong::Song(_song) => {
                let (items, hovered_song_idx) = self.enqueue(self.stack().current().items.iter());
                if !items.is_empty() {
                    let queue_len = ctx.queue.len();
                    let (position, autoplay) = if autoplay {
                        (Position::Replace, Autoplay::Hovered {
                            queue_len,
                            current_song_idx: None,
                            hovered_song_idx,
                        })
                    } else {
                        (Position::EndOfQueue, Autoplay::None)
                    };
                    ctx.command(move |client| {
                        client.enqueue_multiple(items, position, autoplay)?;
                        Ok(())
                    });
                }
            }
        }

        Ok(())
    }
}

impl Pane for PlaylistsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        self.browser.set_filter_input_active(self.filter_input_mode).render(
            area,
            frame.buffer_mut(),
            &mut self.stack,
            ctx,
        );

        Ok(())
    }

    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        if !self.initialized {
            let compare = StringCompare::from(ctx.config.browser_song_sort.as_ref());
            ctx.query().id(INIT).target(PaneType::Playlists).replace_id(INIT).query(
                move |client| {
                    let result: Vec<_> = client
                        .list_playlists()
                        .context("Cannot list playlists")?
                        .into_iter()
                        .sorted_by(|a, b| compare.compare(&a.name, &b.name))
                        .map(|playlist| DirOrSong::playlist_name_only(playlist.name))
                        .collect();
                    Ok(MpdQueryResult::DirOrSong { data: result, path: None })
                },
            );

            self.initialized = true;
        }
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, _is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::Database | UiEvent::StoredPlaylist => {
                let id = match event {
                    UiEvent::Database => INIT,
                    UiEvent::StoredPlaylist => REINIT,
                    _ => return Ok(()),
                };

                let sort_opts = ctx.config.browser_song_sort.clone();
                ctx.query().id(id).replace_id(id).target(PaneType::Playlists).query(
                    move |client| {
                        let result: Vec<_> = client
                            .list_playlists()
                            .context("Cannot list playlists")?
                            .into_iter()
                            .sorted_by(|a, b| {
                                StringCompare::from(sort_opts.as_ref()).compare(&a.name, &b.name)
                            })
                            .map(|playlist| DirOrSong::playlist_name_only(playlist.name))
                            .collect();
                        Ok(MpdQueryResult::DirOrSong { data: result, path: None })
                    },
                );
            }
            UiEvent::Reconnected => {
                self.initialized = false;
                self.before_show(ctx)?;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        self.handle_mouse_action(event, ctx)
    }

    fn handle_action(&mut self, event: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        self.handle_filter_input(event, ctx)?;
        self.handle_common_action(event, ctx)?;
        self.handle_global_action(event, ctx)?;
        Ok(())
    }

    fn on_query_finished(
        &mut self,
        id: &'static str,
        mpd_command: MpdQueryResult,
        _is_visible: bool,
        ctx: &Ctx,
    ) -> Result<()> {
        match (id, mpd_command) {
            (PLAYLIST_INFO, MpdQueryResult::SongsList { data, .. }) => {
                modal!(
                    ctx,
                    InfoListModal::builder()
                        .column_widths(&[30, 70])
                        .title("Playlist info")
                        .items(data)
                        .size((40, 20))
                        .build()
                );
                ctx.render()?;
            }
            (FETCH_DATA, MpdQueryResult::DirOrSong { data, path }) => {
                let Some(path) = path else {
                    log::error!(path:?, current_path:? = self.stack().path(); "Cannot insert data because path is not provided");
                    return Ok(());
                };

                self.stack_mut().insert(path, data);
                self.fetch_data_internal(ctx)?;
                ctx.render()?;
            }
            (INIT, MpdQueryResult::DirOrSong { data, path: _ }) => {
                self.stack = DirStack::new(data);
                if let Some(sel) = self.stack.current().selected() {
                    self.fetch_data(sel, ctx)?;
                }
                ctx.render()?;
            }
            (REINIT, MpdQueryResult::DirOrSong { data, .. }) => {
                let mut new_stack = DirStack::new(data);
                let old_viewport_len = self.stack.current().state.viewport_len();
                let old_content_len = self.stack.current().state.content_len();
                let old_marked = self.stack.current().marked().clone();
                match self.stack.path().as_slice() {
                    [playlist_name] => {
                        let Some((selected_idx, selected_playlist)) =
                            self.stack().previous().map(|prev| {
                                prev.selected_with_idx()
                                    .map_or((0, playlist_name.as_str()), |(idx, playlist)| {
                                        (idx, playlist.as_path())
                                    })
                            })
                        else {
                            log::error!(stack:? = self.stack(); "Reinitializing playlists. Current path sugsests that we are inside a playlist but previous is None");
                            return Ok(());
                        };

                        let idx_to_select = new_stack
                            .current()
                            .items
                            .iter()
                            .find_position(|item| item.as_path() == selected_playlist)
                            .map_or(selected_idx, |(idx, _)| idx);

                        new_stack.current_mut().state.set_viewport_len(old_viewport_len); // is this needed?
                        new_stack
                            .current_mut()
                            .state
                            .select(Some(idx_to_select), ctx.config.scrolloff);

                        log::debug!(stack:? = new_stack; "Reinitializing playlist stack");

                        let selected_song = self.stack().current().selected_with_idx();

                        // Get the actually selected playlist after the resolution. If none is
                        // selected then it means that we have no more playlists to go into so we
                        // end here
                        let Some(new_playlist) = new_stack.current().selected() else {
                            return Ok(());
                        };

                        let playlist = new_playlist.as_path().to_owned();
                        new_stack.current_mut().state.set_content_len(old_content_len);
                        new_stack.current_mut().state.set_viewport_len(old_viewport_len);

                        let songs = ctx.query_sync(move |client| {
                            Ok(client.list_playlist_info(&playlist, None)?)
                        })?;

                        // Calculate next path based on the playlist that was selected, can be
                        // either the same playlist by name or the same index. If no playllist is
                        // selected, ie the stack is empty, we end here
                        let Some(next_path) = new_stack.next_path() else {
                            log::debug!(stack:? = new_stack; "No playlist selected after reinit, not entering");
                            return Ok(());
                        };
                        new_stack
                            .insert(next_path, songs.into_iter().map(DirOrSong::Song).collect());
                        new_stack.enter();

                        if let Some((idx, song)) = selected_song {
                            let idx_to_select = new_stack
                                .current()
                                .items
                                .iter()
                                .find_position(|item| item.as_path() == song.as_path())
                                .map_or(idx, |(idx, _)| idx);
                            new_stack.current_mut().state.set_viewport_len(old_viewport_len);
                            new_stack
                                .current_mut()
                                .state
                                .select(Some(idx_to_select), ctx.config.scrolloff);
                        }
                        *new_stack.current_mut().marked_mut() = old_marked;

                        self.stack = new_stack;
                        ctx.render()?;
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
                            .select(Some(idx_to_select), ctx.config.scrolloff);

                        self.stack = new_stack;
                        if let Some(sel) = self.stack.current().selected() {
                            self.fetch_data(sel, ctx)?;
                        }
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
    fn stack(&self) -> &DirStack<DirOrSong, ListState> {
        &self.stack
    }

    fn stack_mut(&mut self) -> &mut DirStack<DirOrSong, ListState> {
        &mut self.stack
    }

    fn browser_areas(&self) -> EnumMap<BrowserArea, Rect> {
        self.browser.areas
    }

    fn set_filter_input_mode_active(&mut self, active: bool) {
        self.filter_input_mode = active;
    }

    fn is_filter_input_mode_active(&self) -> bool {
        self.filter_input_mode
    }

    fn next(&mut self, ctx: &Ctx) -> Result<()> {
        self.open_or_play(false, ctx)
    }

    fn list_songs_in_item(
        &self,
        item: DirOrSong,
    ) -> impl FnOnce(&mut Client<'_>) -> Result<Vec<Song>> + Clone + 'static {
        move |client| {
            Ok(match item {
                DirOrSong::Dir { name, .. } => client.list_playlist_info(&name, None)?,
                DirOrSong::Song(song) => vec![song.clone()],
            })
        }
    }

    fn fetch_data(&self, selected: &DirOrSong, ctx: &Ctx) -> Result<()> {
        match self.stack.path().as_slice() {
            [] => {
                let DirOrSong::Dir { name: playlist, .. } = selected else {
                    log::error!(selected:? = selected; "Expected playlist to be selected");
                    return Ok(());
                };
                let path = self.stack.next_path();
                let playlist = playlist.to_owned();
                ctx.query()
                    .id(FETCH_DATA)
                    .replace_id("playlists_data")
                    .target(PaneType::Playlists)
                    .query(move |client| {
                        let data = client
                            .list_playlist_info(&playlist, None)?
                            .into_iter()
                            .map(DirOrSong::Song)
                            .collect_vec();
                        Ok(MpdQueryResult::DirOrSong { data, path })
                    });
            }
            _ => {}
        }

        Ok(())
    }

    fn open(&mut self, ctx: &Ctx) -> Result<()> {
        self.open_or_play(true, ctx)
    }

    fn show_info(&self, item: &DirOrSong, ctx: &Ctx) -> Result<()> {
        match item {
            DirOrSong::Dir { name, .. } => {
                let playlist = name.clone();
                ctx.query()
                    .target(PaneType::Playlists)
                    .replace_id(PLAYLIST_INFO)
                    .id(PLAYLIST_INFO)
                    .query(move |client| {
                        let playlist = client.list_playlist_info(&playlist, None)?;
                        Ok(MpdQueryResult::SongsList { data: playlist, path: None })
                    });
            }
            DirOrSong::Song(_) => {}
        }
        Ok(())
    }

    fn delete<'a>(&self, items: impl Iterator<Item = (usize, &'a DirOrSong)>) -> Vec<MpdDelete> {
        match self.stack().path().as_slice() {
            [playlist] => {
                let playlist: Arc<str> = Arc::from(playlist.as_str());
                items
                    .filter_map(|(idx, item)| match item {
                        DirOrSong::Dir { .. } => None,
                        DirOrSong::Song(_) => Some(MpdDelete::SongInPlaylist {
                            playlist: Arc::clone(&playlist),
                            range: SingleOrRange::single(idx),
                        }),
                    })
                    .collect_vec()
            }
            [] => items
                .filter_map(|(_, item)| match item {
                    DirOrSong::Dir { name, .. } => Some(MpdDelete::Playlist { name: name.clone() }),
                    DirOrSong::Song(_) => None,
                })
                .collect_vec(),
            _ => Vec::new(),
        }
    }

    fn can_rename(&self, item: &DirOrSong) -> bool {
        matches!(item, DirOrSong::Dir { .. })
    }

    fn rename(item: &DirOrSong, ctx: &Ctx) -> Result<()> {
        match item {
            DirOrSong::Dir { name: d, .. } => {
                let current_name = d.clone();
                modal!(
                    ctx,
                    InputModal::new(ctx)
                        .title("Rename playlist")
                        .confirm_label("Rename")
                        .input_label("New name:")
                        .initial_value(current_name.clone())
                        .on_confirm(move |ctx, new_value| {
                            if current_name != new_value {
                                let current_name = current_name.clone();
                                let new_value = new_value.to_owned();
                                ctx.command(move |client| {
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

    fn move_selected(&mut self, direction: MoveDirection, ctx: &Ctx) -> Result<()> {
        let Some(DirOrSong::Dir { name: playlist, .. }) =
            self.stack.previous().and_then(|p| p.selected())
        else {
            return Ok(());
        };

        if self.stack().current().marked().is_empty() {
            let Some(idx) = self.stack().current().selected_with_idx().map(|(idx, _)| idx) else {
                status_warn!("Cannot move because no item is selected");
                return Ok(());
            };

            let new_idx = match direction {
                MoveDirection::Up => idx.saturating_sub(1),
                MoveDirection::Down => (idx + 1).min(self.stack().current().items.len() - 1),
            };

            let playlist = playlist.clone();
            ctx.query_sync(move |client| {
                client.move_in_playlist(&playlist, &SingleOrRange::single(idx), new_idx)?;
                Ok(())
            })?;
            self.stack_mut().current_mut().items.swap(idx, new_idx);
            self.stack_mut().current_mut().select_idx(new_idx, ctx.config.scrolloff);
        } else {
            match direction {
                MoveDirection::Up => {
                    if let Some(0) = self.stack().current().marked().first() {
                        return Ok(());
                    }
                }
                MoveDirection::Down => {
                    if let Some(last_idx) = self.stack().current().marked().last()
                        && *last_idx == self.stack().current().items.len() - 1
                    {
                        return Ok(());
                    }
                }
            }

            let playlist = playlist.clone();
            let ranges = self.stack().current().marked().ranges().collect_vec();

            ctx.query_sync(move |client| {
                for range in ranges {
                    let idx = range.start();
                    let new_idx = match direction {
                        MoveDirection::Up => idx.saturating_sub(1),
                        MoveDirection::Down => idx + 1,
                    };
                    client.move_in_playlist(&playlist, &(range.into()), new_idx)?;
                }

                Ok(())
            })?;

            let mut new_marked = BTreeSet::new();
            for marked in self.stack().current().marked() {
                match direction {
                    MoveDirection::Up => {
                        new_marked.insert(marked.saturating_sub(1));
                    }
                    MoveDirection::Down => {
                        new_marked.insert(*marked + 1);
                    }
                }
            }

            *self.stack_mut().current_mut().marked_mut() = new_marked;

            return Ok(());
        }
        ctx.render()?;

        Ok(())
    }
}
