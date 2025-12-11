use anyhow::Result;
use enum_map::EnumMap;
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect, widgets::ListState};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::tabs::PaneType,
    ctx::Ctx,
    mpd::{
        client::Client,
        commands::Song,
        mpd_client::{Filter, FilterKind, MpdClient, Tag},
    },
    shared::{key_event::KeyEvent, mouse_event::MouseEvent, mpd_client_ext::Enqueue},
    ui::{
        UiEvent,
        browser::BrowserPane,
        dir_or_song::DirOrSong,
        dirstack::DirStack,
        input::InputResultEvent,
        widgets::browser::{Browser, BrowserArea},
    },
};

#[derive(Debug)]
pub struct DirectoriesPane {
    stack: DirStack<DirOrSong, ListState>,
    browser: Browser<DirOrSong>,
    initialized: bool,
}

const INIT: &str = "init";
const FETCH_DATA: &str = "fetch_data";

impl DirectoriesPane {
    pub fn new(_ctx: &Ctx) -> Self {
        Self { stack: DirStack::default(), browser: Browser::new(), initialized: false }
    }
}

impl Pane for DirectoriesPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> anyhow::Result<()> {
        self.browser.render(area, frame.buffer_mut(), &mut self.stack, ctx);

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
                    Ok(MpdQueryResult::DirOrSong { data: result, path: None })
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

    fn handle_insert_mode(&mut self, kind: InputResultEvent, ctx: &mut Ctx) -> Result<()> {
        log::debug!(kind:?; "Handling insert mode in DirectoriesPane");
        BrowserPane::handle_insert_mode(self, kind, ctx)?;
        Ok(())
    }

    fn handle_action(&mut self, event: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
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
                self.fetch_data_internal(ctx)?;
                ctx.render()?;
            }
            _ => {}
        }
        Ok(())
    }
}

impl BrowserPane<DirOrSong> for DirectoriesPane {
    fn stack(&self) -> &DirStack<DirOrSong, ListState> {
        &self.stack
    }

    fn stack_mut(&mut self) -> &mut DirStack<DirOrSong, ListState> {
        &mut self.stack
    }

    fn browser_areas(&self) -> EnumMap<BrowserArea, Rect> {
        self.browser.areas
    }

    fn list_songs_in_item(
        &self,
        item: DirOrSong,
    ) -> impl FnOnce(&mut Client<'_>) -> Result<Vec<Song>> + Clone + 'static {
        move |client| {
            Ok(match item {
                DirOrSong::Dir { full_path, playlist: false, .. } => {
                    client.find(&[Filter::new_with_kind(
                        Tag::File,
                        &full_path,
                        FilterKind::StartsWith,
                    )])?
                }
                DirOrSong::Dir { name, playlist: true, .. } => {
                    client.list_playlist_info(&name, None)?
                }
                DirOrSong::Song(song) => vec![song.clone()],
            })
        }
    }

    fn fetch_data(&self, selected: &DirOrSong, ctx: &Ctx) -> Result<()> {
        match selected {
            DirOrSong::Dir { playlist: is_playlist, .. } => {
                let Some(next_path) = self.stack.next_path() else {
                    log::error!("Failed to move deeper inside dir. Next path is None");
                    return Ok(());
                };

                let is_playlist = *is_playlist;
                let playlist_display_mode = ctx.config.show_playlists_in_browser;

                let sort = ctx.config.directories_sort.clone();
                ctx.query()
                    .id(FETCH_DATA)
                    .replace_id("directories_data")
                    .target(PaneType::Directories)
                    .query(move |client| {
                        let data: Vec<_> = if is_playlist {
                            client
                                .list_playlist_info(&next_path.to_string(), None)?
                                .into_iter()
                                .map(DirOrSong::Song)
                                .collect()
                        } else {
                            match client.lsinfo(Some(&next_path.to_string())) {
                                Ok(val) => val,
                                Err(err) => {
                                    log::error!(error:? = err; "Failed to get lsinfo for dir",);
                                    return Ok(MpdQueryResult::DirOrSong {
                                        data: Vec::new(),
                                        path: None,
                                    });
                                }
                            }
                            .0
                            .into_iter()
                            .filter_map(|v| v.into_dir_or_song(playlist_display_mode))
                            .sorted_by(|a, b| {
                                a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort))
                            })
                            .collect()
                        };

                        Ok(MpdQueryResult::DirOrSong { data, path: Some(next_path) })
                    });
            }
            DirOrSong::Song(_) => {}
        }
        Ok(())
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
                    .find(|(_, path)| path == &&curr.dir_name_or_file())
                    .map(|(idx, _)| idx)
            } else {
                None
            }
        };

        (items, hovered_idx)
    }
}
