use anyhow::Result;
use enum_map::EnumMap;
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::{keys::actions::Position, tabs::PaneType},
    ctx::Ctx,
    mpd::{
        client::Client,
        commands::Song,
        mpd_client::{Filter, FilterKind, MpdClient, Tag},
    },
    shared::{
        key_event::KeyEvent,
        mouse_event::MouseEvent,
        mpd_client_ext::{Autoplay, Enqueue, MpdClientExt},
        mpd_query::PreviewGroup,
    },
    ui::{
        UiEvent,
        browser::BrowserPane,
        dir_or_song::DirOrSong,
        dirstack::{DirStack, DirStackItem},
        widgets::browser::{Browser, BrowserArea},
    },
};

#[derive(Debug)]
pub struct DirectoriesPane {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
    browser: Browser<DirOrSong>,
    initialized: bool,
}

const INIT: &str = "init";
const OPEN_OR_PLAY: &str = "open_or_play";
const PREVIEW: &str = "preview";

impl DirectoriesPane {
    pub fn new(_ctx: &Ctx) -> Self {
        Self {
            stack: DirStack::default(),
            filter_input_mode: false,
            browser: Browser::new(),
            initialized: false,
        }
    }

    fn open_or_play(&mut self, autoplay: bool, ctx: &Ctx) -> Result<()> {
        let Some(selected) = self.stack.current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };
        let Some(next_path) = self.stack.next_path() else {
            log::error!("Failed to move deeper inside dir. Next path is None");
            return Ok(());
        };

        let sort = ctx.config.directories_sort.clone();
        match selected {
            DirOrSong::Dir { playlist: is_playlist, .. } => {
                let is_playlist = *is_playlist;
                let playlist_display_mode = ctx.config.show_playlists_in_browser;
                ctx.query()
                    .id(OPEN_OR_PLAY)
                    .replace_id(OPEN_OR_PLAY)
                    .target(PaneType::Directories)
                    .query(move |client| {
                        let data = if is_playlist {
                            client
                                .list_playlist_info(&next_path.join("/").to_string(), None)?
                                .into_iter()
                                .map(DirOrSong::Song)
                                .sorted_by(|a, b| {
                                    a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort))
                                })
                                .collect()
                        } else {
                            client
                                .lsinfo(Some(&next_path.join("/").to_string()))?
                                .into_iter()
                                .filter_map(|v| v.into_dir_or_song(playlist_display_mode))
                                .sorted_by(|a, b| {
                                    a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort))
                                })
                                .collect()
                        };

                        Ok(MpdQueryResult::DirOrSong { data, origin_path: Some(next_path) })
                    });
                self.stack_mut().push(Vec::new());
                self.stack_mut().clear_preview();
                ctx.render()?;
            }
            DirOrSong::Song(_) => {
                let (items, hovered_song_idx) = self.enqueue(
                    self.stack()
                        .current()
                        .items
                        .iter()
                        // Only add songs here in case the directory contains combination of
                        // directories, playlists and songs to be able to use autoplay from the
                        // hovered song properly.
                        .filter(|item| matches!(item, DirOrSong::Song(_))),
                );
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

impl Pane for DirectoriesPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> anyhow::Result<()> {
        self.browser.set_filter_input_active(self.filter_input_mode).render(
            area,
            frame.buffer_mut(),
            &mut self.stack,
            &ctx.config,
        );

        Ok(())
    }

    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        if !self.initialized {
            let sort = ctx.config.directories_sort.clone();
            let playlist_display_mode = ctx.config.show_playlists_in_browser;
            ctx.query().id(INIT).replace_id(INIT).target(PaneType::Directories).query(
                move |client| {
                    let result = client
                        .lsinfo(None)?
                        .into_iter()
                        .filter_map(|v| v.into_dir_or_song(playlist_display_mode))
                        .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
                        .collect::<Vec<_>>();
                    Ok(MpdQueryResult::DirOrSong { data: result, origin_path: None })
                },
            );
            self.initialized = true;
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, _is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::Database => {
                let sort = ctx.config.directories_sort.clone();
                let playlist_display_mode = ctx.config.show_playlists_in_browser;
                ctx.query().id(INIT).replace_id(INIT).target(PaneType::Directories).query(
                    move |client| {
                        let result = client
                            .lsinfo(None)?
                            .into_iter()
                            .filter_map(|v| v.into_dir_or_song(playlist_display_mode))
                            .sorted_by(|a, b| {
                                a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort))
                            })
                            .collect::<Vec<_>>();
                        Ok(MpdQueryResult::DirOrSong { data: result, origin_path: None })
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
        data: MpdQueryResult,
        _is_visible: bool,
        ctx: &Ctx,
    ) -> Result<()> {
        match (id, data) {
            (PREVIEW, MpdQueryResult::Preview { data, origin_path }) => {
                if let Some(origin_path) = origin_path
                    && origin_path != self.stack().path()
                {
                    log::trace!(origin_path:?, current_path:? = self.stack().path(); "Dropping preview because it does not belong to this path");
                    return Ok(());
                }
                self.stack_mut().set_preview(data);
                ctx.render()?;
            }
            (INIT, MpdQueryResult::DirOrSong { data, origin_path: _ }) => {
                self.stack = DirStack::new(data);
                self.prepare_preview(ctx)?;
                ctx.render()?;
            }
            (OPEN_OR_PLAY, MpdQueryResult::DirOrSong { data, origin_path }) => {
                if let Some(origin_path) = origin_path
                    && origin_path != self.stack().path()
                {
                    log::trace!(origin_path:?, current_path:? = self.stack().path(); "Dropping result because it does not belong to this path");
                    return Ok(());
                }
                self.stack_mut().replace(data);
                self.prepare_preview(ctx)?;
                ctx.render()?;
            }
            _ => {}
        }
        Ok(())
    }
}

impl BrowserPane<DirOrSong> for DirectoriesPane {
    fn stack(&self) -> &DirStack<DirOrSong> {
        &self.stack
    }

    fn stack_mut(&mut self) -> &mut DirStack<DirOrSong> {
        &mut self.stack
    }

    fn initial_playlist_name(&self) -> Option<String> {
        self.stack().current().selected().and_then(|item| match item {
            DirOrSong::Dir { name, .. } => Some(name.to_owned()),
            DirOrSong::Song(_) => None,
        })
    }

    fn set_filter_input_mode_active(&mut self, active: bool) {
        self.filter_input_mode = active;
    }

    fn is_filter_input_mode_active(&self) -> bool {
        self.filter_input_mode
    }

    fn list_songs_in_item(
        &self,
        item: DirOrSong,
    ) -> impl FnOnce(&mut Client<'_>) -> Result<Vec<Song>> + Clone + 'static {
        move |client| {
            Ok(match item {
                DirOrSong::Dir { full_path, .. } => client.find(&[Filter::new_with_kind(
                    Tag::File,
                    &full_path,
                    FilterKind::StartsWith,
                )])?,
                DirOrSong::Song(song) => vec![song.clone()],
            })
        }
    }

    fn enqueue<'a>(
        &self,
        items: impl Iterator<Item = &'a DirOrSong>,
    ) -> (Vec<Enqueue>, Option<usize>) {
        let mut dir_or_playlist_found = false;
        let items = items
            .map(|item| match item {
                DirOrSong::Dir { full_path, playlist: true, .. } => {
                    dir_or_playlist_found = true;
                    Enqueue::Playlist { name: full_path.to_owned() }
                }
                DirOrSong::Dir { full_path, playlist: false, .. } => {
                    dir_or_playlist_found = true;
                    Enqueue::File { path: full_path.to_owned() }
                }
                DirOrSong::Song(song) => Enqueue::File { path: song.file.clone() },
            })
            .collect_vec();

        let hovered_idx = if dir_or_playlist_found {
            None
        } else {
            // We are not adding any playlists or directories so autoplay on hovered item
            // can work
            if let Some(curr) = self.stack().current().selected() {
                items
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, item)| {
                        if let Enqueue::File { path } = item { Some((idx, path)) } else { None }
                    })
                    .find(|(_, path)| path == &&curr.dir_name_or_file_name())
                    .map(|(idx, _)| idx)
            } else {
                None
            }
        };

        (items, hovered_idx)
    }

    fn open(&mut self, ctx: &Ctx) -> Result<()> {
        self.open_or_play(true, ctx)
    }

    fn next(&mut self, ctx: &Ctx) -> Result<()> {
        self.open_or_play(false, ctx)
    }

    fn prepare_preview(&mut self, ctx: &Ctx) -> Result<()> {
        let origin_path = Some(self.stack().path().to_vec());
        match &self.stack.current().selected() {
            Some(DirOrSong::Dir { playlist: is_playlist, .. }) => {
                let Some(next_path) = self.stack.next_path() else {
                    log::error!("Failed to move deeper inside dir. Next path is None");
                    return Ok(());
                };
                let next_path = next_path.join("/").to_string();
                let config = std::sync::Arc::clone(&ctx.config);
                let sort = ctx.config.directories_sort.clone();
                let is_playlist = *is_playlist;
                let playlist_display_mode = ctx.config.show_playlists_in_browser;

                self.stack_mut().clear_preview();
                ctx.query()
                    .id(PREVIEW)
                    .replace_id("directories_preview")
                    .target(PaneType::Directories)
                    .query(move |client| {
                        let data: Vec<_> = if is_playlist {
                            client
                                .list_playlist_info(&next_path, None)?
                                .into_iter()
                                .map(DirOrSong::Song)
                                .sorted_by(|a, b| {
                                    a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort))
                                })
                                .map(|v| v.to_list_item_simple(&config))
                                .collect()
                        } else {
                            match client.lsinfo(Some(&next_path)) {
                                Ok(val) => val,
                                Err(err) => {
                                    log::error!(error:? = err; "Failed to get lsinfo for dir",);
                                    return Ok(MpdQueryResult::Preview {
                                        data: None,
                                        origin_path: None,
                                    });
                                }
                            }
                            .0
                            .into_iter()
                            .filter_map(|v| v.into_dir_or_song(playlist_display_mode))
                            .sorted_by(|a, b| {
                                a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort))
                            })
                            .map(|v| v.to_list_item_simple(&config))
                            .collect()
                        };

                        Ok(MpdQueryResult::Preview {
                            data: Some(vec![PreviewGroup::from(None, None, data)]),
                            origin_path,
                        })
                    });
            }
            Some(DirOrSong::Song(song)) => {
                let file = song.file.clone();
                let key_style = ctx.config.theme.preview_label_style;
                let group_style = ctx.config.theme.preview_metadata_group_style;
                ctx.query()
                    .id(PREVIEW)
                    .replace_id("directories_preview")
                    .target(PaneType::Directories)
                    .query(move |client| {
                        Ok(MpdQueryResult::Preview {
                            data: client
                                .find_one(&[Filter::new(Tag::File, &file)])?
                                .map(|v| v.to_preview(key_style, group_style)),
                            origin_path,
                        })
                    });
            }
            None => {}
        }
        Ok(())
    }

    fn browser_areas(&self) -> EnumMap<BrowserArea, Rect> {
        self.browser.areas
    }
}
