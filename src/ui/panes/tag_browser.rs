use std::{cmp::Ordering, sync::Arc};

use anyhow::{Context, Result};
use enum_map::EnumMap;
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect, widgets::ListState};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::{
        artists::{AlbumDisplayMode, AlbumSortMode},
        tabs::PaneType,
    },
    ctx::Ctx,
    mpd::{
        client::Client,
        commands::Song,
        mpd_client::{Filter, FilterKind, MpdClient, Tag},
    },
    shared::{
        cmp::StringCompare,
        key_event::KeyEvent,
        mouse_event::MouseEvent,
        string_util::StringExt,
    },
    ui::{
        UiEvent,
        browser::BrowserPane,
        dir_or_song::DirOrSong,
        dirstack::{DirStack, DirStackItem, Path},
        input::{BufferId, InputResultEvent},
        widgets::browser::{Browser, BrowserArea},
    },
};

#[derive(Debug)]
pub struct TagBrowserPane {
    stack: DirStack<DirOrSong, ListState>,
    root_tag: Tag,
    separator: Option<Arc<str>>,
    unescaped_separator: Option<String>,
    target_pane: PaneType,
    browser: Browser<DirOrSong>,
    initialized: bool,
    input_buffer_id: BufferId,
}

const INIT: &str = "init";
const FETCH_SONGS: &str = "fetch_songs";

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
            browser: Browser::new(),
            initialized: false,
            input_buffer_id: BufferId::new(),
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

    fn process_songs(&mut self, artist: String, data: Vec<Song>, ctx: &Ctx) {
        let display_mode = ctx.config.artists.album_display_mode;
        let sort_mode = ctx.config.artists.album_sort_by;

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

                (album, song_date)
            })
            .into_iter()
            .sorted_by(|((album_a, date_a), _), ((album_b, date_b), _)| match sort_mode {
                AlbumSortMode::Name => match album_a.cmp(album_b) {
                    Ordering::Equal => date_a.cmp(date_b),
                    ordering => ordering,
                },
                AlbumSortMode::Date => date_a.cmp(date_b),
            })
            .map(|((album, date), mut songs)| {
                songs.sort_by(|a, b| {
                    a.with_custom_sort(&ctx.config.browser_song_sort)
                        .cmp(&b.with_custom_sort(&ctx.config.browser_song_sort))
                });

                let name = match display_mode {
                    AlbumDisplayMode::SplitByDate => {
                        format!("({date}) {album}")
                    }
                    AlbumDisplayMode::NameOnly => album.clone(),
                };
                (name, songs)
            })
            .fold(Vec::new(), |mut acc, album| {
                match display_mode {
                    AlbumDisplayMode::SplitByDate => {
                        acc.push(album);
                    }
                    AlbumDisplayMode::NameOnly => {
                        if let Some(cached_album) =
                            acc.iter_mut().find(|cached_album| cached_album.0 == album.0)
                        {
                            cached_album.1.extend(album.1);
                        } else {
                            acc.push(album);
                        }
                    }
                }
                acc
            });

        let path: Path = artist.into();
        self.stack.insert(
            path.clone(),
            albums.iter().map(|album| DirOrSong::name_only(album.0.clone())).collect(),
        );

        for album in albums {
            let album_path = path.join(album.0);
            self.stack.insert(album_path, album.1.into_iter().map(DirOrSong::Song).collect());
        }
    }
}

impl Pane for TagBrowserPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        self.browser.render(area, frame.buffer_mut(), &mut self.stack, ctx);

        Ok(())
    }

    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        if !self.initialized {
            let root_tag = self.root_tag.clone();
            let target = self.target_pane.clone();
            ctx.query().id(INIT).replace_id(INIT).target(target).query(move |client| {
                let result = client.list_tag(root_tag, None).context("Cannot list artists")?;
                Ok(MpdQueryResult::LsInfo { data: result.0, path: None })
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
                self.stack = DirStack::default();
                ctx.query().id(INIT).replace_id(INIT).target(target).query(move |client| {
                    let result = client.list_tag(root_tag, None).context("Cannot list artists")?;
                    Ok(MpdQueryResult::LsInfo { data: result.0, path: None })
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

    fn handle_insert_mode(&mut self, kind: InputResultEvent, ctx: &mut Ctx) -> Result<()> {
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
            (FETCH_SONGS, MpdQueryResult::SongsList { data, path }) => {
                let Some(root_path) = path.and_then(|v| v.as_slice().iter().next().cloned()) else {
                    return Ok(());
                };

                self.process_songs(root_path, data, ctx);
                self.fetch_data_internal(ctx)?;
                ctx.render()?;
            }
            (INIT, MpdQueryResult::LsInfo { data, path: _ }) => {
                let sort_opts = ctx.config.browser_song_sort.as_ref();

                let data = if let Some(sep) = &self.unescaped_separator {
                    data.into_iter()
                        .flat_map(|item| item.split(sep.as_str()).map(str::to_string).collect_vec())
                        .unique()
                        .sorted_by(|a, b| StringCompare::from(sort_opts).compare(a, b))
                        .map(DirOrSong::name_only)
                        .collect_vec()
                } else {
                    data.into_iter()
                        .sorted_by(|a, b| StringCompare::from(sort_opts).compare(a, b))
                        .map(DirOrSong::name_only)
                        .collect_vec()
                };

                self.stack = DirStack::new(data);
                if let Some(sel) = self.stack.current().selected() {
                    self.fetch_data(sel, ctx)?;
                }
                ctx.render()?;
            }
            _ => {}
        }
        Ok(())
    }
}

impl BrowserPane<DirOrSong> for TagBrowserPane {
    fn buffer_id(&self) -> BufferId {
        self.input_buffer_id
    }

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
        let root_tag = self.root_tag.clone();
        let separator = self.separator.clone();
        let path = self.stack().path().to_owned();

        let album_songs = match self.stack.path().as_slice() {
            [_artist] => self
                .stack
                .next_dir_items()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| match item {
                            DirOrSong::Dir { .. } => None,
                            DirOrSong::Song(song) => Some(song.clone()),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };

        move |client| {
            Ok(match item {
                DirOrSong::Dir { name, .. } => match path.as_slice() {
                    [_artist] => album_songs,
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

    fn fetch_data(&self, selected: &DirOrSong, ctx: &Ctx) -> Result<()> {
        match self.stack.path().as_slice() {
            [_artist, _album] => {
                ctx.render()?;
            }
            [_artist] => {
                ctx.render()?;
            }
            [] => {
                let current = selected.as_path();
                let root_tag = self.root_tag.clone();
                let separator = self.separator.clone();
                let target = self.target_pane.clone();
                let current = current.to_owned();

                ctx.query().id(FETCH_SONGS).replace_id(FETCH_SONGS).target(target).query(
                    move |client| {
                        let separator = separator.map(|v| v.as_ref().to_owned());
                        let separator = separator.as_deref();
                        let all_songs: Vec<Song> =
                            client.find(&[Self::root_tag_filter(root_tag, separator, &current)])?;
                        Ok(MpdQueryResult::SongsList {
                            data: all_songs,
                            path: Some(current.into()),
                        })
                    },
                );
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

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
            last_modified: chrono::Utc::now(),
            added: None,
        }
    }

    fn pane_albums(pane: &TagBrowserPane) -> Vec<String> {
        pane.stack
            .get(&"artist".into())
            .expect("expected artist dir to exist")
            .items
            .iter()
            .map(|item| item.as_path().to_owned())
            .collect_vec()
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

        pane.process_songs(artist.clone(), songs, &ctx);

        assert_eq!(pane.stack.contained_paths().sorted().collect_vec(), vec![
            &Path::from([]),
            &Path::from("artist"),
            &Path::from(["artist", "album_a"]),
            &Path::from(["artist", "album_b"]),
        ]);
        assert_eq!(pane_albums(&pane), vec!["album_a", "album_b"]);
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

        pane.process_songs(artist.clone(), songs, &ctx);

        assert_eq!(pane.stack.contained_paths().sorted().collect_vec(), vec![
            &Path::from([]),
            &Path::from("artist"),
            &Path::from(["artist", "(2020) album_a"]),
            &Path::from(["artist", "(2021) album_a"]),
            &Path::from(["artist", "(2022) album_b"]),
        ]);
        assert_eq!(pane_albums(&pane), vec!["(2020) album_a", "(2021) album_a", "(2022) album_b"]);
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

        pane.process_songs(artist.clone(), songs, &ctx);

        assert_eq!(pane.stack.contained_paths().sorted().collect_vec(), vec![
            &Path::from([]),
            &Path::from("artist"),
            &Path::from(["artist", "(2019) album_b"]),
            &Path::from(["artist", "(2020) album_a"]),
            &Path::from(["artist", "(2021) album_a"]),
        ]);
        assert_eq!(pane_albums(&pane), vec!["(2019) album_b", "(2020) album_a", "(2021) album_a"]);
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

        pane.process_songs(artist.clone(), songs, &ctx);

        assert_eq!(pane.stack.contained_paths().sorted().collect_vec(), vec![
            &Path::from([]),
            &Path::from("artist"),
            &Path::from(["artist", "album_a"]),
            &Path::from(["artist", "album_b"]),
        ]);
        assert_eq!(pane_albums(&pane), vec!["album_b", "album_a"]);
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

        pane.process_songs(artist.clone(), songs, &ctx);

        assert_eq!(pane.stack.contained_paths().sorted().collect_vec(), vec![
            &Path::from([]),
            &Path::from("artist"),
            &Path::from(["artist", "(1969) album_a"]), // Uses originaldate, not date
            &Path::from(["artist", "(1970) album_b"]), // Uses originaldate, not date
        ]);
        assert_eq!(pane_albums(&pane), vec!["(1969) album_a", "(1970) album_b"]);
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

        pane.process_songs(artist.clone(), songs, &ctx);

        assert_eq!(pane.stack.contained_paths().sorted().collect_vec(), vec![
            &Path::from([]),
            &Path::from("artist"),
            &Path::from(["artist", "(1969) album_a"]), // Uses originaldate (first in list)
            &Path::from(["artist", "(1990) album_b"]), // Falls back to date (second in list)
        ]);
        assert_eq!(pane_albums(&pane), vec!["(1969) album_a", "(1990) album_b"]);
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

        pane.process_songs(artist.clone(), songs, &ctx);

        assert_eq!(pane.stack.contained_paths().sorted().collect_vec(), vec![
            &Path::from([]),
            &Path::from("artist"),
            &Path::from(["artist", "(<no date>) album_a"]) // Falls back to default
        ]);
        assert_eq!(pane_albums(&pane), vec!["(<no date>) album_a"]);
    }
}
