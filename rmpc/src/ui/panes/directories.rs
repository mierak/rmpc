use anyhow::Result;
use enum_map::EnumMap;
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect, widgets::ListState};
use rmpc_mpd::{
    client::Client,
    commands::Song,
    filter::{Filter, FilterKind, Tag},
    mpd_client::MpdClient,
};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::{sort_mode::SortOptions, tabs::PaneType},
    ctx::Ctx,
    shared::{keys::ActionEvent, mouse_event::MouseEvent, mpd_client_ext::Enqueue},
    ui::{
        UiEvent,
        browser::BrowserPane,
        dir_or_song::{DirOrSong, LsInfoEntryExt as _},
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
        BrowserPane::handle_insert_mode(self, kind, ctx)?;
        Ok(())
    }

    fn handle_action(&mut self, event: &mut ActionEvent, ctx: &mut Ctx) -> Result<()> {
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
                                    log::error!(error:? = err; "Failed to get lsinfo for dir");
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
                    Enqueue::Directory { path: full_path.to_owned() }
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

    fn resolve_enqueue(&self, items: Vec<Enqueue>, ctx: &Ctx) -> Result<Vec<Enqueue>> {
        if !items.iter().any(|item| matches!(item, Enqueue::Directory { .. })) {
            return Ok(items);
        }

        let sort = ctx.config.directories_sort.clone();
        let playlist_mode = ctx.config.show_playlists_in_browser;
        ctx.query_sync(move |client| {
            let mut list_dir = |path: &str| -> Result<Vec<DirOrSong>> {
                Ok(client
                    .lsinfo(Some(path))?
                    .0
                    .into_iter()
                    .filter_map(|entry| entry.into_dir_or_song(playlist_mode))
                    .collect())
            };

            let mut resolved = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Enqueue::Directory { path } => {
                        resolve_directory(&path, &sort, &mut list_dir, &mut resolved)?;
                    }
                    other => resolved.push(other),
                }
            }
            Ok(resolved)
        })
    }
}

// Expand a directory the way the pane shows it: sort each level on its own and
// descend depth first, so songs stay grouped by their directory instead of
// being flattened into a single sort. Songs, subdirectories and playlists at a
// level are all handled.
fn resolve_directory<F>(
    path: &str,
    sort: &SortOptions,
    list_dir: &mut F,
    out: &mut Vec<Enqueue>,
) -> Result<()>
where
    F: FnMut(&str) -> Result<Vec<DirOrSong>>,
{
    let mut entries = list_dir(path)?;
    entries.sort_by(|a, b| a.with_custom_sort(sort).cmp(&b.with_custom_sort(sort)));
    for entry in entries {
        match entry {
            DirOrSong::Song(song) => out.push(Enqueue::File { path: song.file }),
            DirOrSong::Dir { full_path, playlist: true, .. } => {
                out.push(Enqueue::Playlist { name: full_path });
            }
            DirOrSong::Dir { full_path, playlist: false, .. } => {
                resolve_directory(&full_path, sort, list_dir, out)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::HashMap;

    use rmpc_mpd::commands::{Song, metadata_tag::MetadataTag};

    use super::resolve_directory;
    use crate::{
        config::{
            sort_mode::{SortMode, SortOptions},
            theme::properties::SongProperty,
        },
        shared::mpd_client_ext::Enqueue,
        ui::dir_or_song::DirOrSong,
    };

    fn song(file: &str, track: &str) -> DirOrSong {
        DirOrSong::Song(Song {
            id: 0,
            file: file.to_string(),
            duration: None,
            metadata: HashMap::from([(
                "track".to_string(),
                MetadataTag::Single(track.to_string()),
            )]),
            last_modified: chrono::Utc::now(),
            added: None,
        })
    }

    fn dir(full_path: &str) -> DirOrSong {
        DirOrSong::Dir {
            name: full_path.rsplit('/').next().unwrap_or(full_path).to_string(),
            display_name: None,
            full_path: full_path.to_string(),
            last_modified: chrono::Utc::now(),
            playlist: false,
            metadata: HashMap::new(),
        }
    }

    fn playlist(full_path: &str) -> DirOrSong {
        DirOrSong::Dir {
            name: full_path.to_string(),
            display_name: None,
            full_path: full_path.to_string(),
            last_modified: chrono::Utc::now(),
            playlist: true,
            metadata: HashMap::new(),
        }
    }

    fn sort_by_track() -> SortOptions {
        SortOptions {
            mode: SortMode::Format(vec![SongProperty::Track]),
            group_by_type: true,
            reverse: false,
            ignore_leading_the: false,
            fold_case: true,
        }
    }

    fn resolve(root: &str, tree: &HashMap<&str, Vec<DirOrSong>>) -> Vec<String> {
        let mut list_dir = |path: &str| -> anyhow::Result<Vec<DirOrSong>> {
            Ok(tree.get(path).cloned().unwrap_or_default())
        };
        let mut out = Vec::new();
        resolve_directory(root, &sort_by_track(), &mut list_dir, &mut out).unwrap();
        out.into_iter()
            .map(|item| match item {
                Enqueue::File { path } => path,
                Enqueue::Playlist { name } => name,
                other => panic!("unexpected enqueue entry: {other:?}"),
            })
            .collect()
    }

    // Artist/ holds Album1 and Album2, each with tracks out of order. Adding
    // Artist keeps every album's tracks together and sorted within the album
    // instead of interleaving track 1 of both albums, then track 2, etc.
    #[test]
    fn groups_songs_by_directory() {
        let tree = HashMap::from([
            ("Artist", vec![dir("Artist/Album2"), dir("Artist/Album1")]),
            ("Artist/Album1", vec![
                song("Artist/Album1/2.flac", "2"),
                song("Artist/Album1/1.flac", "1"),
            ]),
            ("Artist/Album2", vec![
                song("Artist/Album2/2.flac", "2"),
                song("Artist/Album2/1.flac", "1"),
            ]),
        ]);

        assert_eq!(resolve("Artist", &tree), vec![
            "Artist/Album1/1.flac",
            "Artist/Album1/2.flac",
            "Artist/Album2/1.flac",
            "Artist/Album2/2.flac",
        ]);
    }

    // A level mixing a subdirectory, loose songs and a playlist keeps the pane's
    // order: with group_by_type the directory expands first, then the songs,
    // then the playlist.
    #[test]
    fn handles_mixed_directory() {
        let tree = HashMap::from([
            ("Mixed", vec![
                song("Mixed/2.flac", "2"),
                playlist("Mixed/list"),
                dir("Mixed/Sub"),
                song("Mixed/1.flac", "1"),
            ]),
            ("Mixed/Sub", vec![song("Mixed/Sub/a.flac", "1")]),
        ]);

        assert_eq!(resolve("Mixed", &tree), vec![
            "Mixed/Sub/a.flac",
            "Mixed/1.flac",
            "Mixed/2.flac",
            "Mixed/list",
        ]);
    }
}
