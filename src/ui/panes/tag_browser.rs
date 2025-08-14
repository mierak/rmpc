use std::{cmp::Ordering, collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use enum_map::EnumMap;
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::{
        artists::{AlbumDisplayMode, AlbumSortMode},
        keys::actions::Position,
        tabs::PaneType,
    },
    ctx::Ctx,
    mpd::{
        client::Client,
        commands::{Song, metadata_tag::MetadataTagExt},
        mpd_client::{Filter, FilterKind, MpdClient, Tag},
    },
    shared::{
        key_event::KeyEvent,
        mouse_event::MouseEvent,
        mpd_client_ext::{Autoplay, Enqueue, MpdClientExt},
        mpd_query::PreviewGroup,
        string_util::StringExt,
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
pub struct TagBrowserPane {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
    root_tag: Tag,
    separator: Option<Arc<str>>,
    unescaped_separator: Option<String>,
    target_pane: PaneType,
    browser: Browser<DirOrSong>,
    initialized: bool,
    cache: TagBrowserCache,
}

const INIT: &str = "init";
const OPEN_OR_PLAY: &str = "open_or_play";
const PREVIEW: &str = "preview";

#[derive(Debug, Default)]
struct TagBrowserCache(HashMap<String, CachedRootTag>);

#[derive(Debug, Default)]
struct CachedRootTag(Vec<CachedAlbum>);

#[derive(Debug, Default)]
struct CachedAlbum {
    name: String,
    original_name: String,
    songs: Vec<Song>,
}

impl TagBrowserPane {
    pub fn new(
        root_tag: Tag,
        target_pane: PaneType,
        separator: Option<String>,
        _ctx: &Ctx,
    ) -> Self {
        Self {
            root_tag,
            target_pane,
            separator: separator.as_ref().map(|sep| sep.escape_regex_chars().into()),
            unescaped_separator: separator,
            stack: DirStack::default(),
            filter_input_mode: false,
            browser: Browser::new(),
            initialized: false,
            cache: TagBrowserCache::default(),
        }
    }

    fn root_tag_filter<'value>(
        root_tag: Tag,
        separator: Option<&str>,
        value: &'value str,
    ) -> Filter<'value> {
        match separator {
            None => Filter::new(root_tag, value),
            Some(_) if value.is_empty() => Filter::new(root_tag, value),
            // Exact match search cannot be used when separator is present because a single item in
            // the list might be only part of the whole tag value. Thus we search for the value
            // prependend by either start of the line or *anything* followed by the separator and
            // followed by either end of the line or *anything* followed by the separator again.
            Some(separator) => Filter::new_with_kind(
                root_tag,
                format!("(^|.*{separator}){value}($|{separator}.*)"),
                FilterKind::Regex,
            ),
        }
    }

    fn open_or_play(&mut self, autoplay: bool, ctx: &Ctx) -> Result<()> {
        let Some(current) = self.stack.current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };
        let Some(_next_path) = self.stack.next_path() else {
            log::error!("Failed to move deeper inside dir. Next path is None");
            return Ok(());
        };

        match self.stack.path() {
            [_artist, _album] => {
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
            [artist] => {
                let Some(albums) = self.cache.0.get(artist) else {
                    return Ok(());
                };
                let Some(CachedAlbum { songs, .. }) =
                    albums.0.iter().find(|album| album.name == current.as_path())
                else {
                    return Ok(());
                };
                let songs =
                    songs.iter().map(|song| DirOrSong::Song(song.clone())).collect::<Vec<_>>();

                self.stack_mut().push(songs);
                ctx.render()?;
            }
            [] => {
                let current = current.as_path().to_owned();
                if let Some(albums) = self.cache.0.get(&current) {
                    let albums = albums
                        .0
                        .iter()
                        .map(|CachedAlbum { name, .. }| DirOrSong::name_only(name.to_owned()))
                        .collect();
                    self.stack_mut().push(albums);
                } else {
                    let root_tag = self.root_tag.clone();
                    let separator = self.separator.clone();
                    let target = self.target_pane.clone();
                    ctx.query().id(OPEN_OR_PLAY).replace_id(OPEN_OR_PLAY).target(target).query(
                        move |client| {
                            let root_tag_filter =
                                Self::root_tag_filter(root_tag, separator.as_deref(), &current);
                            let all_songs: Vec<Song> = client.find(&[root_tag_filter])?;
                            Ok(MpdQueryResult::SongsList {
                                data: all_songs,
                                origin_path: Some(vec![current]),
                            })
                        },
                    );
                    self.stack_mut().push(Vec::new());
                    self.stack_mut().clear_preview();
                    ctx.render()?;
                }
            }
            _ => {
                log::error!("Unexpected nesting in Artists dir structure");
            }
        }

        Ok(())
    }

    fn process_songs(&mut self, artist: String, data: Vec<Song>, ctx: &Ctx) -> &CachedRootTag {
        let display_mode = ctx.config.artists.album_display_mode;
        let sort_mode = ctx.config.artists.album_sort_by;

        let cached_artist = self.cache.0.entry(artist).or_default();

        let albums = data
            .into_iter()
            .into_group_map_by(|song| {
                let album = song.metadata.get("album").map_or("<no album>".to_string(), |v| {
                    v.join(&ctx.config.theme.format_tag_separator).to_string()
                });
                let song_date = ctx
                    .config
                    .artists
                    .album_date_tags
                    .iter()
                    .find_map(|tag| {
                        song.metadata
                            .get(Into::<&'static str>::into(tag))
                            .map(|v| v.last().to_string())
                    })
                    .unwrap_or_else(|| "<no date>".to_string());
                let original_album = song.metadata.get("album").last().map(|v| v.to_owned());

                (album, song_date, original_album)
            })
            .into_iter()
            .sorted_by(|((album_a, date_a, _), _), ((album_b, date_b, _), _)| match sort_mode {
                AlbumSortMode::Name => match album_a.cmp(album_b) {
                    Ordering::Equal => date_a.cmp(date_b),
                    ordering => ordering,
                },
                AlbumSortMode::Date => date_a.cmp(date_b),
            })
            .map(|((album, date, original_name), mut songs)| {
                songs.sort_by(|a, b| {
                    a.with_custom_sort(&ctx.config.browser_song_sort)
                        .cmp(&b.with_custom_sort(&ctx.config.browser_song_sort))
                });

                CachedAlbum {
                    name: match display_mode {
                        AlbumDisplayMode::SplitByDate => {
                            format!("({date}) {album}")
                        }
                        AlbumDisplayMode::NameOnly => album.to_string(),
                    },
                    original_name: original_name.unwrap_or_else(String::new),
                    songs,
                }
            })
            .fold(Vec::new(), |mut acc, album| {
                match display_mode {
                    AlbumDisplayMode::SplitByDate => {
                        acc.push(album);
                    }
                    AlbumDisplayMode::NameOnly => {
                        if let Some(cached_album) =
                            acc.iter_mut().find(|cached_album| cached_album.name == album.name)
                        {
                            cached_album.songs.extend(album.songs);
                        } else {
                            acc.push(album);
                        }
                    }
                }
                acc
            });

        cached_artist.0 = albums;

        cached_artist
    }
}

impl Pane for TagBrowserPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
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
            let root_tag = self.root_tag.clone();
            let target = self.target_pane.clone();
            ctx.query().id(INIT).replace_id(INIT).target(target).query(move |client| {
                let result = client.list_tag(root_tag, None).context("Cannot list artists")?;
                Ok(MpdQueryResult::LsInfo { data: result.0, origin_path: None })
            });

            self.initialized = true;
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, _is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::Database => {
                let root_tag = self.root_tag.clone();
                let target = self.target_pane.clone();
                self.cache = TagBrowserCache::default();
                ctx.query().id(INIT).replace_id(INIT).target(target).query(move |client| {
                    let result = client.list_tag(root_tag, None).context("Cannot list artists")?;
                    Ok(MpdQueryResult::LsInfo { data: result.0, origin_path: None })
                });
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
            (PREVIEW, MpdQueryResult::SongsList { data, origin_path }) => {
                let Some(artist) = origin_path.and_then(|mut v| v.first_mut().map(std::mem::take))
                else {
                    return Ok(());
                };

                let current_item_path = self.stack().current().selected().map(|c| c.as_path());
                // We still want to cache the result to avoid refetch later, but
                // do not rerender current state because rmpc is
                // already on a different item
                let cache_only = if current_item_path == Some(&artist) {
                    false
                } else {
                    log::trace!(artist:?, current_item_path:?; "Dropping preview because it does not belong to this path");
                    true
                };

                let cached_artist = self.process_songs(artist, data, ctx);

                if cache_only {
                    return Ok(());
                }

                let preview = vec![PreviewGroup::from(
                    None,
                    None,
                    cached_artist
                        .0
                        .iter()
                        .map(|album| {
                            DirOrSong::name_only(album.name.clone())
                                .to_list_item_simple(&ctx.config)
                        })
                        .collect(),
                )];
                self.stack.set_preview(Some(preview));
                ctx.render()?;
            }
            (OPEN_OR_PLAY, MpdQueryResult::SongsList { data, origin_path }) => {
                let Some(artist) = origin_path.and_then(|mut v| v.first_mut().map(std::mem::take))
                else {
                    return Ok(());
                };

                // We still want to cache the result to avoid refetch later, but
                // do not rerender current state because rmpc is
                // already on a different item
                let cache_only = if self.stack().path().first() == Some(&artist) {
                    false
                } else {
                    log::trace!(artist:?, current_path:? = self.stack().path(); "Dropping result because it does not belong to this path");
                    true
                };

                if cache_only {
                    return Ok(());
                }

                let cached_artist = self.process_songs(artist, data, ctx);

                let albums = cached_artist
                    .0
                    .iter()
                    .map(|CachedAlbum { name, .. }| DirOrSong::name_only(name.to_owned()))
                    .collect();
                self.stack.replace(albums);
                self.prepare_preview(ctx)?;
                ctx.render()?;
            }
            (INIT, MpdQueryResult::LsInfo { data, origin_path: _ }) => {
                let data = if let Some(sep) = &self.unescaped_separator {
                    data.into_iter()
                        .flat_map(|item| item.split(sep.as_str()).map(str::to_string).collect_vec())
                        .unique()
                        .sorted()
                        .map(DirOrSong::name_only)
                        .collect_vec()
                } else {
                    data.into_iter().sorted().map(DirOrSong::name_only).collect_vec()
                };

                self.stack = DirStack::new(data);
                self.prepare_preview(ctx)?;
                ctx.render()?;
            }
            _ => {}
        }
        Ok(())
    }
}

impl BrowserPane<DirOrSong> for TagBrowserPane {
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
        let root_tag = self.root_tag.clone();
        let separator = self.separator.clone();
        let path = self.stack().path().to_owned();
        let album_name = match (self.stack().path(), &item) {
            ([artist], DirOrSong::Dir { name, .. }) => self
                .cache
                .0
                .get(artist)
                .and_then(|albums| {
                    albums.0.iter().find(|a| &a.name == name).map(|a| a.original_name.clone())
                })
                .unwrap_or_default(),
            _ => String::new(),
        };

        move |client| {
            Ok(match item {
                DirOrSong::Dir { name, .. } => match path.as_slice() {
                    [artist] => client.find(&[
                        Filter::new(Tag::Album, &album_name),
                        Self::root_tag_filter(root_tag, separator.as_deref(), artist),
                    ])?,
                    [] => client.find(&[Self::root_tag_filter(
                        root_tag,
                        separator.as_deref(),
                        &name,
                    )])?,
                    _ => Vec::new(),
                },
                DirOrSong::Song(song) => vec![song.clone()],
            })
        }
    }

    fn enqueue<'a>(
        &self,
        items: impl Iterator<Item = &'a DirOrSong>,
    ) -> (Vec<Enqueue>, Option<usize>) {
        match self.stack.path() {
            [_tag_value, _album] => {
                let hovered =
                    self.stack.current().selected().map(|item| item.dir_name_or_file_name());

                items.enumerate().fold((Vec::new(), None), |mut acc, (idx, item)| {
                    let filename = item.dir_name_or_file_name().into_owned();
                    if hovered.as_ref().is_some_and(|hovered| hovered == &filename) {
                        acc.1 = Some(idx);
                    }
                    acc.0.push(Enqueue::Find {
                        filter: vec![(Tag::File, FilterKind::Exact, filename)],
                    });

                    acc
                })
            }
            [tag_value] => {
                let tag_value = tag_value.clone();
                let Some(albums) = self.cache.0.get(&tag_value) else {
                    return (Vec::new(), None);
                };

                let items = items
                    .filter_map(|item| {
                        let name = item.dir_name_or_file_name();
                        albums.0.iter().find(|a| a.name == name)
                    })
                    .flat_map(|album| {
                        album.songs.iter().map(|song| Enqueue::Find {
                            filter: vec![(Tag::File, FilterKind::Exact, song.file.clone())],
                        })
                    })
                    .collect_vec();

                (items, None)
            }
            [] => {
                let root_tag = self.root_tag.clone();
                let separator = self.separator.clone();

                (
                    items
                        .map(|item| item.dir_name_or_file_name().into_owned())
                        .map(|name| {
                            let mut filter = Self::root_tag_filter(
                                root_tag.clone(),
                                separator.as_deref(),
                                &name,
                            );
                            Enqueue::Find {
                                filter: vec![(
                                    filter.tag,
                                    filter.kind,
                                    std::mem::take(&mut filter.value).into_owned(),
                                )],
                            }
                        })
                        .collect_vec(),
                    None,
                )
            }
            _ => (Vec::new(), None),
        }
    }

    fn open(&mut self, ctx: &Ctx) -> Result<()> {
        self.open_or_play(true, ctx)
    }

    fn next(&mut self, ctx: &Ctx) -> Result<()> {
        self.open_or_play(false, ctx)
    }

    fn prepare_preview(&mut self, ctx: &Ctx) -> Result<()> {
        let Some(current) = self.stack.current().selected().map(DirStackItem::as_path) else {
            return Ok(());
        };
        let current = current.to_owned();

        self.stack_mut().clear_preview();
        match self.stack.path() {
            [artist, album] => {
                let key_style = ctx.config.theme.preview_label_style;
                let group_style = ctx.config.theme.preview_metadata_group_style;
                let Some(albums) = self.cache.0.get(artist) else {
                    return Ok(());
                };
                let Some(CachedAlbum { songs, .. }) = albums.0.iter().find(|a| &a.name == album)
                else {
                    return Ok(());
                };
                let song = songs
                    .iter()
                    .find(|song| song.file == current)
                    .map(|song| song.to_preview(key_style, group_style));
                self.stack_mut().set_preview(song);
                ctx.render()?;
            }
            [artist] => {
                let Some(albums) = self.cache.0.get(artist) else {
                    return Ok(());
                };
                let Some(CachedAlbum { songs, .. }) =
                    albums.0.iter().find(|album| album.name == current)
                else {
                    return Ok(());
                };
                let songs = vec![PreviewGroup::from(
                    None,
                    None,
                    songs.iter().map(|song| song.to_list_item_simple(&ctx.config)).collect_vec(),
                )];
                self.stack_mut().set_preview(Some(songs));
                ctx.render()?;
            }
            [] => {
                if let Some(albums) = self.cache.0.get(&current) {
                    self.stack.set_preview(Some(vec![PreviewGroup::from(
                        None,
                        None,
                        albums
                            .0
                            .iter()
                            .map(|CachedAlbum { name, .. }| {
                                DirOrSong::name_only(name.to_owned())
                                    .to_list_item_simple(&ctx.config)
                            })
                            .collect(),
                    )]));
                    ctx.render()?;
                } else {
                    let root_tag = self.root_tag.clone();
                    let separator = self.separator.clone();
                    let target = self.target_pane.clone();
                    ctx.query().id(PREVIEW).replace_id(PREVIEW).target(target).query(
                        move |client| {
                            let separator = separator.map(|v| v.as_ref().to_owned());
                            let separator = separator.as_deref();
                            let all_songs: Vec<Song> = client
                                .find(&[Self::root_tag_filter(root_tag, separator, &current)])?;
                            Ok(MpdQueryResult::SongsList {
                                data: all_songs,
                                origin_path: Some(vec![current]),
                            })
                        },
                    );
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn browser_areas(&self) -> EnumMap<BrowserArea, Rect> {
        self.browser.areas
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        config::{Config, artists::AlbumDateTag},
        tests::fixtures::{config, ctx},
    };

    fn song(
        album: impl Into<String> + std::fmt::Debug,
        date: impl Into<String> + std::fmt::Debug,
    ) -> Song {
        Song {
            id: 0,
            file: format!("{date:?} {album:?}"),
            duration: None,
            metadata: HashMap::from([
                ("album".to_string(), Into::<String>::into(album).into()),
                ("date".to_string(), Into::<String>::into(date).into()),
            ]),
            stickers: None,
            last_modified: chrono::Utc::now(),
            added: None,
        }
    }

    fn song_with_originaldate(
        album: impl Into<String> + std::fmt::Debug,
        date: impl Into<String> + std::fmt::Debug,
        original_date: impl Into<String> + std::fmt::Debug,
    ) -> Song {
        Song {
            id: 0,
            file: format!("{date:?} {album:?}"),
            duration: None,
            metadata: HashMap::from([
                ("album".to_string(), Into::<String>::into(album).into()),
                ("date".to_string(), Into::<String>::into(date).into()),
                ("originaldate".to_string(), Into::<String>::into(original_date).into()),
            ]),
            stickers: None,
            last_modified: chrono::Utc::now(),
            added: None,
        }
    }

    #[rstest]
    fn albums_no_date_sort_name(mut ctx: Ctx, mut config: Config) {
        config.artists.album_display_mode = AlbumDisplayMode::NameOnly;
        config.artists.album_sort_by = AlbumSortMode::Name;
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(Tag::Artist, PaneType::Artists, None, &ctx);
        let artist = String::from("artist");
        let songs = vec![
            song("album_a", "2020"),
            song("album_b", "2022"),
            song("album_a", "2021"),
            song("album_b", "2022"),
        ];

        let CachedRootTag(result) = pane.process_songs(artist, songs, &ctx);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "album_a");
        assert_eq!(result[1].name, "album_b");
    }

    #[rstest]
    fn albums_split_date_sort_name(mut ctx: Ctx, mut config: Config) {
        config.artists.album_display_mode = AlbumDisplayMode::SplitByDate;
        config.artists.album_sort_by = AlbumSortMode::Name;
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(Tag::Artist, PaneType::Artists, None, &ctx);
        let artist = String::from("artist");
        let songs = vec![
            song("album_a", "2020"),
            song("album_b", "2022"),
            song("album_a", "2021"),
            song("album_b", "2022"),
        ];

        let CachedRootTag(result) = pane.process_songs(artist, songs, &ctx);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].name, "(2020) album_a");
        assert_eq!(result[1].name, "(2021) album_a");
        assert_eq!(result[2].name, "(2022) album_b");
    }

    #[rstest]
    fn albums_split_date_sort_date(mut ctx: Ctx, mut config: Config) {
        config.artists.album_display_mode = AlbumDisplayMode::SplitByDate;
        config.artists.album_sort_by = AlbumSortMode::Date;
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(Tag::Artist, PaneType::Artists, None, &ctx);
        let artist = String::from("artist");
        let songs = vec![
            song("album_a", "2020"),
            song("album_b", "2019"),
            song("album_a", "2021"),
            song("album_b", "2019"),
        ];

        let CachedRootTag(result) = pane.process_songs(artist, songs, &ctx);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].name, "(2019) album_b");
        assert_eq!(result[1].name, "(2020) album_a");
        assert_eq!(result[2].name, "(2021) album_a");
    }

    #[rstest]
    fn albums_no_date_sort_date(mut ctx: Ctx, mut config: Config) {
        config.artists.album_display_mode = AlbumDisplayMode::NameOnly;
        config.artists.album_sort_by = AlbumSortMode::Date;
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(Tag::Artist, PaneType::Artists, None, &ctx);
        let artist = String::from("artist");
        let songs = vec![
            song("album_a", "2020"),
            song("album_b", "2019"),
            song("album_a", "2021"),
            song("album_b", "2025"),
        ];

        let CachedRootTag(result) = pane.process_songs(artist, songs, &ctx);
        dbg!(&result);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "album_b");
        assert_eq!(result[1].name, "album_a");
    }

    #[rstest]
    fn albums_single_configured_tag(mut ctx: Ctx, mut config: Config) {
        config.artists.album_display_mode = AlbumDisplayMode::SplitByDate;
        config.artists.album_sort_by = AlbumSortMode::Date;
        config.artists.album_date_tags = vec![AlbumDateTag::OriginalDate];
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(Tag::Artist, PaneType::Artists, None, &ctx);
        let artist = String::from("artist");
        let songs = vec![
            song_with_originaldate("album_a", "1987", "1969"), /* remastered in 1987, original
                                                                * from 1969 */
            song_with_originaldate("album_b", "1990", "1970"), /* remastered in 1990, original
                                                                * from 1970 */
        ];

        let CachedRootTag(result) = pane.process_songs(artist, songs, &ctx);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "(1969) album_a"); // Uses originaldate, not date
        assert_eq!(result[1].name, "(1970) album_b"); // Uses originaldate, not date
    }

    #[rstest]
    fn albums_tag_fallback(mut ctx: Ctx, mut config: Config) {
        config.artists.album_display_mode = AlbumDisplayMode::SplitByDate;
        config.artists.album_sort_by = AlbumSortMode::Date;
        config.artists.album_date_tags = vec![AlbumDateTag::OriginalDate, AlbumDateTag::Date];
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(Tag::Artist, PaneType::Artists, None, &ctx);
        let artist = String::from("artist");
        let songs = vec![
            song_with_originaldate("album_a", "1987", "1969"), // Has both originaldate and date
            song("album_b", "1990"),                           // Only has date, not originaldate
        ];

        let CachedRootTag(result) = pane.process_songs(artist, songs, &ctx);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "(1969) album_a"); // Uses originaldate (first in list)
        assert_eq!(result[1].name, "(1990) album_b"); // Falls back to date (second in list)
    }

    #[rstest]
    fn albums_no_matching_tags(mut ctx: Ctx, mut config: Config) {
        config.artists.album_display_mode = AlbumDisplayMode::SplitByDate;
        config.artists.album_sort_by = AlbumSortMode::Date;
        config.artists.album_date_tags = vec![AlbumDateTag::OriginalDate];
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(Tag::Artist, PaneType::Artists, None, &ctx);
        let artist = String::from("artist");
        let songs = vec![
            song("album_a", "1987"), // Only has "date", not in our list
        ];

        let CachedRootTag(result) = pane.process_songs(artist, songs, &ctx);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "(<no date>) album_a"); // Falls back to default
    }
}
