use anyhow::{Context, Result};
use enum_map::EnumMap;
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect, widgets::ListState};
use rmpc_mpd::{
    client::Client,
    commands::Song,
    filter::{Filter, FilterKind, Tag},
    mpd_client::MpdClient,
};
use rmpc_shared::string_ext::StringExt;

use super::Pane;
use crate::{
    MpdQueryResult,
    config::tabs::{BrowserTagConfig, PaneType},
    ctx::Ctx,
    shared::{cmp::StringCompare, keys::ActionEvent, mouse_event::MouseEvent},
    ui::{
        UiEvent,
        browser::BrowserPane,
        dir_or_song::DirOrSong,
        dirstack::{DirStack, DirStackItem, Path, WalkDirStackItem},
        input::InputResultEvent,
        song_ext::SongExt as _,
        widgets::browser::{Browser, BrowserArea},
    },
};

#[derive(Debug)]
pub struct TagBrowserPane {
    stack: DirStack<DirOrSong, ListState>,
    tags: Vec<BrowserTagConfig>,
    target_pane: PaneType,
    browser: Browser<DirOrSong>,
    initialized: bool,
}

const INIT: &str = "init";
const FETCH_SONGS: &str = "fetch_songs";

impl TagBrowserPane {
    pub fn new(tags: Vec<BrowserTagConfig>, target_pane: PaneType, _ctx: &Ctx) -> Self {
        Self {
            tags,
            target_pane,
            stack: DirStack::default(),
            browser: Browser::new(),
            initialized: false,
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

    fn group_songs_by_tag(
        songs: Vec<Song>,
        tag: &BrowserTagConfig,
        ctx: &Ctx,
    ) -> Vec<(String, Vec<Song>)> {
        let sep = ctx.config.theme.format_tag_separator.as_str();
        let sort_opts = &ctx.config.browser_song_sort;

        let tag_of = |song: &Song| {
            song.metadata
                .get(&tag.tag)
                .map_or_else(|| format!("<no {}>", tag.tag), |v| v.join(sep).into_owned())
        };

        let first_tag_of = |song: &Song, tags: &[String]| {
            tags.iter().find_map(|tag| song.metadata.get(tag).map(|v| v.last().to_string()))
        };

        let split_tags = tag.split_by_tag.as_deref().unwrap_or(&[]);

        let mut groups: Vec<(String, Option<String>, Option<String>, Vec<Song>)> = songs
            .into_iter()
            .into_group_map_by(|s| (tag_of(s), first_tag_of(s, split_tags)))
            .into_iter()
            .flat_map(|((name, split_tag), mut songs)| {
                songs.sort_by(|a, b| {
                    a.with_custom_sort(sort_opts).cmp(&b.with_custom_sort(sort_opts))
                });

                // There can be songs with different sort tag values within the
                // same group. Pick the first one as the sanest
                // approach, this will never be perfect but should
                // be good enough in most cases.
                let sort_tag = tag
                    .sort_by
                    .as_ref()
                    .and_then(|t| songs.iter().find_map(|s| first_tag_of(s, t)));

                let names = if let Some(tag_separator) = &tag.separator {
                    name.split(tag_separator).map(str::to_string).unique().collect_vec()
                } else {
                    vec![name]
                };

                names
                    .into_iter()
                    .map(move |name| (name, split_tag.clone(), sort_tag.clone(), songs.clone()))
            })
            .collect_vec();

        groups.sort_by(
            |(name_a, split_tag_a, sort_tag_a, _), (name_b, split_tag_b, sort_tag_b, _)| match tag
                .sort_by
            {
                None => name_a.cmp(name_b).then_with(|| split_tag_a.cmp(split_tag_b)),
                Some(_) => sort_tag_a.cmp(sort_tag_b).then_with(|| name_a.cmp(name_b)),
            },
        );

        groups
            .into_iter()
            .map(|(name, split_tag, _, songs)| {
                let display_name = match &tag.split_by_tag {
                    Some(tags) => format!(
                        "({}) {name}",
                        split_tag.unwrap_or_else(|| format!("<no {}>", tags.join(sep)))
                    ),
                    None => name,
                };
                (display_name, songs)
            })
            .collect()
    }

    fn insert_level(
        stack: &mut DirStack<DirOrSong, ListState>,
        current_path: Path,
        songs: Vec<Song>,
        remaining_tags: &[BrowserTagConfig],
        ctx: &Ctx,
    ) {
        if remaining_tags.is_empty() {
            let sort_opts = ctx.config.browser_song_sort.as_ref();
            let mut songs = songs;
            songs.sort_by(|a, b| a.with_custom_sort(sort_opts).cmp(&b.with_custom_sort(sort_opts)));
            stack.insert(current_path, songs.into_iter().map(DirOrSong::Song).collect());
            return;
        }

        let tag = &remaining_tags[0];
        let rest = &remaining_tags[1..];

        let groups = Self::group_songs_by_tag(songs, tag, ctx);

        stack.insert(
            current_path.clone(),
            groups.iter().map(|(name, _)| DirOrSong::name_only(name.clone())).collect(),
        );

        for (name, group_songs) in groups {
            let child_path = current_path.join(&name);
            Self::insert_level(stack, child_path, group_songs, rest, ctx);
        }
    }

    fn process_songs(&mut self, root_value: String, data: Vec<Song>, ctx: &Ctx) {
        Self::insert_level(&mut self.stack, root_value.into(), data, &self.tags[1..], ctx);
    }

    fn songs_for_item(&self, item: &DirOrSong) -> Vec<Song> {
        let path = self.stack().path().to_owned();
        item.walk(&self.stack, path)
            .filter_map(|item| match item {
                DirOrSong::Song(song) => Some(song.clone()),
                DirOrSong::Dir { .. } => None,
            })
            .collect()
    }
}

impl Pane for TagBrowserPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        self.browser.render(area, frame.buffer_mut(), &mut self.stack, ctx);

        Ok(())
    }

    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        if !self.initialized {
            let root_tag = self.tags[0].tag.clone();
            let target = self.target_pane.clone();
            ctx.query().id(INIT).replace_id(INIT).target(target).query(move |client| {
                let result =
                    client.list_tag(Tag::Custom(root_tag), None).context("Cannot list artists")?;
                log::debug!("Fetched root tag values: {:?}", result.0);
                Ok(MpdQueryResult::LsInfo { data: result.0, path: None })
            });

            self.initialized = true;
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, _is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::Database => {
                let root_tag = self.tags[0].tag.clone();
                let target = self.target_pane.clone();
                self.stack = DirStack::default();
                ctx.query().id(INIT).replace_id(INIT).target(target).query(move |client| {
                    let result = client
                        .list_tag(Tag::Custom(root_tag), None)
                        .context("Cannot list artists")?;
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

                let data = if let Some(sep) = &self.tags[0].separator {
                    data.into_iter()
                        .flat_map(|item| item.split(sep).map(str::to_string).collect_vec())
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
        let prebuilt_songs = self.songs_for_item(&item);
        move |_client| Ok(prebuilt_songs)
    }

    fn fetch_data(&self, selected: &DirOrSong, ctx: &Ctx) -> Result<()> {
        match self.stack.path().as_slice() {
            [] => {
                let current = selected.as_path();
                let root_tag = Tag::Custom(self.tags[0].tag.clone());
                let escaped_separator =
                    self.tags[0].separator.as_ref().map(|sep| sep.escape_regex_chars());
                let target = self.target_pane.clone();
                let current = current.to_owned();

                ctx.query().id(FETCH_SONGS).replace_id(FETCH_SONGS).target(target).query(
                    move |client| {
                        let separator = escaped_separator.as_deref();
                        let all_songs: Vec<Song> =
                            client.find(&[Self::root_tag_filter(root_tag, separator, &current)])?;
                        Ok(MpdQueryResult::SongsList {
                            data: all_songs,
                            path: Some(current.into()),
                        })
                    },
                );
            }
            _ => {
                ctx.render()?;
            }
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
        config::Config,
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

    fn song_with_disc(
        album: impl Into<String> + std::fmt::Debug,
        date: impl Into<String> + std::fmt::Debug,
        disc: impl Into<String> + std::fmt::Debug,
        n: u32,
    ) -> Song {
        let disc = disc.into();
        Song {
            id: n,
            file: format!("{date:?} {album:?} disc{disc} track{n}"),
            duration: None,
            metadata: HashMap::from([
                ("album".to_string(), album.into().into()),
                ("date".to_string(), date.into().into()),
                ("disc".to_string(), disc.into()),
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

    fn tag(tag: impl Into<String>) -> BrowserTagConfig {
        BrowserTagConfig {
            tag: tag.into(),
            split_by_tag: Some(vec!["date".to_string()]),
            sort_by: Some(vec!["date".to_string()]),
            separator: None,
        }
    }

    fn album_tag(sort_tag: Option<&str>, date_tags: Option<&[&str]>) -> BrowserTagConfig {
        let date_tags = date_tags.map(|tags| tags.iter().map(|tag| tag.to_string()).collect_vec());

        BrowserTagConfig {
            tag: "album".to_string(),
            split_by_tag: date_tags,
            sort_by: sort_tag.map(|tag| vec![tag.to_string()]),
            separator: None,
        }
    }

    fn song_with_multi_album(album: impl Into<String>, date: impl Into<String>, n: u32) -> Song {
        let album = album.into();
        let date = date.into();
        Song {
            id: n,
            file: format!("{date} {album} track{n}"),
            duration: None,
            metadata: HashMap::from([
                ("album".to_string(), album.into()),
                ("date".to_string(), date.into()),
            ]),
            last_modified: chrono::Utc::now(),
            added: None,
        }
    }

    #[rstest]
    fn albums_no_date_sort_name(mut ctx: Ctx, config: Config) {
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(
            vec![tag("artist"), album_tag(None, None)],
            PaneType::Artists,
            &ctx,
        );
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
    fn albums_split_date_sort_name(mut ctx: Ctx, config: Config) {
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(
            vec![tag("artist"), album_tag(None, Some(&["date"]))],
            PaneType::Artists,
            &ctx,
        );
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
    fn albums_split_date_sort_date(mut ctx: Ctx, config: Config) {
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(
            vec![tag("artist"), album_tag(Some("date"), Some(&["date"]))],
            PaneType::Artists,
            &ctx,
        );
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
    fn albums_split_date_sort_date_and_then_sort_name(mut ctx: Ctx, config: Config) {
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(
            vec![tag("artist"), album_tag(Some("date"), Some(&["date"]))],
            PaneType::Artists,
            &ctx,
        );
        let artist = String::from("artist");
        let songs = vec![
            song("album_c", "2020"),
            song("album_b", "2020"),
            song("album_d", "2020"),
            song("album_a", "2020"),
        ];

        pane.process_songs(artist.clone(), songs, &ctx);

        assert_eq!(pane.stack.contained_paths().sorted().collect_vec(), vec![
            &Path::from([]),
            &Path::from("artist"),
            &Path::from(["artist", "(2020) album_a"]),
            &Path::from(["artist", "(2020) album_b"]),
            &Path::from(["artist", "(2020) album_c"]),
            &Path::from(["artist", "(2020) album_d"]),
        ]);
        assert_eq!(pane_albums(&pane), vec![
            "(2020) album_a",
            "(2020) album_b",
            "(2020) album_c",
            "(2020) album_d",
        ]);
    }

    #[rstest]
    fn albums_no_date_sort_date(mut ctx: Ctx, config: Config) {
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(
            vec![tag("artist"), album_tag(Some("date"), None)],
            PaneType::Artists,
            &ctx,
        );
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
    fn albums_single_configured_tag(mut ctx: Ctx, config: Config) {
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(
            vec![tag("artist"), album_tag(Some("date"), Some(&["originaldate"]))],
            PaneType::Artists,
            &ctx,
        );
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
    fn albums_tag_fallback(mut ctx: Ctx, config: Config) {
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(
            vec![tag("artist"), album_tag(Some("date"), Some(&["originaldate", "date"]))],
            PaneType::Artists,
            &ctx,
        );
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
    fn albums_no_matching_tags(mut ctx: Ctx, config: Config) {
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(
            vec![tag("artist"), album_tag(Some("date"), Some(&["originaldate"]))],
            PaneType::Artists,
            &ctx,
        );
        let artist = String::from("artist");
        let songs = vec![
            song("album_a", "1987"), // Only has "date", not in our list
        ];

        pane.process_songs(artist.clone(), songs, &ctx);

        assert_eq!(pane.stack.contained_paths().sorted().collect_vec(), vec![
            &Path::from([]),
            &Path::from("artist"),
            &Path::from(["artist", "(<no originaldate>) album_a"]) // Falls back to default
        ]);
        assert_eq!(pane_albums(&pane), vec!["(<no originaldate>) album_a"]);
    }

    mod list_songs_in_item {
        use super::*;

        #[rstest]
        fn in_song_returns_that_song(mut ctx: Ctx, config: Config) {
            ctx.config = std::sync::Arc::new(config);
            let mut pane = TagBrowserPane::new(
                vec![tag("artist"), album_tag(None, None)],
                PaneType::Artists,
                &ctx,
            );
            let songs = vec![song("album_a", "2020"), song("album_b", "2021")];
            pane.stack.insert(Path::new(), vec![DirOrSong::name_only("artist".to_string())]);

            pane.process_songs("artist".to_string(), songs.clone(), &ctx);
            pane.stack_mut().current_mut().select_idx(0, 0);
            pane.stack_mut().enter();
            pane.stack_mut().enter();

            let result = pane.songs_for_item(&DirOrSong::Song(songs[0].clone()));
            assert_eq!(result, vec![songs[0].clone()]);
        }

        #[rstest]
        fn at_root_returns_all_songs_under_artist(mut ctx: Ctx, config: Config) {
            ctx.config = std::sync::Arc::new(config);
            let mut pane = TagBrowserPane::new(
                vec![tag("artist"), album_tag(None, None)],
                PaneType::Artists,
                &ctx,
            );
            let songs = vec![song("album_a", "2020"), song("album_b", "2021")];
            pane.stack.insert(Path::new(), vec![DirOrSong::name_only("artist".to_string())]);
            pane.process_songs("artist".to_string(), songs.clone(), &ctx);

            let mut result = pane.songs_for_item(&DirOrSong::name_only("artist".to_string()));
            result.sort_by(|a, b| a.file.cmp(&b.file));
            let mut expected = songs.clone();
            expected.sort_by(|a, b| a.file.cmp(&b.file));

            assert_eq!(result, expected);
        }

        #[rstest]
        fn at_artist_level_returns_only_that_albums_songs(mut ctx: Ctx, config: Config) {
            ctx.config = std::sync::Arc::new(config);
            let mut pane = TagBrowserPane::new(
                vec![tag("artist"), album_tag(None, None)],
                PaneType::Artists,
                &ctx,
            );
            let songs_a = vec![song("album_a", "2020"), song("album_a", "2020")];
            let songs_b = vec![song("album_b", "2021")];
            pane.stack.insert(Path::new(), vec![DirOrSong::name_only("artist".to_string())]);
            pane.process_songs("artist".to_string(), [songs_a.clone(), songs_b].concat(), &ctx);
            pane.stack_mut().current_mut().select_idx(0, 0);
            pane.stack_mut().enter();

            let result = pane.songs_for_item(&DirOrSong::name_only("album_a".to_string()));

            dbg!(&pane.stack.current().selected());
            dbg!(&pane.stack.current());
            assert_eq!(result.len(), songs_a.len());
            assert!(result.iter().all(|s| s.file.contains("album_a")));
        }

        #[rstest]
        fn for_nonexistent_dir_returns_empty(mut ctx: Ctx, config: Config) {
            ctx.config = std::sync::Arc::new(config);
            let mut pane = TagBrowserPane::new(
                vec![tag("artist"), album_tag(None, None)],
                PaneType::Artists,
                &ctx,
            );
            pane.stack.insert(Path::new(), vec![DirOrSong::name_only("artist".to_string())]);
            pane.process_songs("artist".to_string(), vec![], &ctx);
            pane.stack_mut().current_mut().select_idx(0, 0);
            pane.stack_mut().enter();

            let result = pane.songs_for_item(&DirOrSong::name_only("nonexistent".to_string()));

            assert!(result.is_empty());
        }

        #[rstest]
        fn at_album_level_with_more_nesting(mut ctx: Ctx, config: Config) {
            ctx.config = std::sync::Arc::new(config);
            let disc_tag = BrowserTagConfig {
                tag: "disc".to_string(),
                split_by_tag: None,
                sort_by: None,
                separator: None,
            };
            let mut pane = TagBrowserPane::new(
                vec![tag("artist"), album_tag(None, None), disc_tag],
                PaneType::Artists,
                &ctx,
            );

            let disc1_songs = vec![
                song_with_disc("album_a", "2020", "1", 1),
                song_with_disc("album_a", "2020", "1", 2),
            ];
            let disc2_songs = vec![
                song_with_disc("album_a", "2020", "2", 3),
                song_with_disc("album_a", "2020", "2", 4),
            ];
            let other_songs = vec![song_with_disc("album_b", "2021", "1", 5)];
            let all_songs = [disc1_songs.clone(), disc2_songs.clone(), other_songs].concat();

            pane.stack.insert(Path::new(), vec![DirOrSong::name_only("artist".to_string())]);
            pane.process_songs("artist".to_string(), all_songs, &ctx);
            pane.stack_mut().current_mut().select_idx(0, 0);
            pane.stack_mut().enter();

            let result = pane.songs_for_item(&DirOrSong::name_only("album_a".to_string()));

            let expected_count = disc1_songs.len() + disc2_songs.len();
            assert_eq!(result.len(), expected_count);
            assert!(result.iter().all(|s| s.file.contains("album_a")));
        }
    }

    mod separator {
        use super::*;

        #[rstest]
        fn splits_multi_value_album_into_separate_entries(mut ctx: Ctx, config: Config) {
            ctx.config = std::sync::Arc::new(config);
            let mut pane = TagBrowserPane::new(
                vec![tag("artist"), BrowserTagConfig {
                    tag: "album".to_string(),
                    split_by_tag: None,
                    sort_by: None,
                    separator: Some("/".to_string()),
                }],
                PaneType::Artists,
                &ctx,
            );
            let songs = vec![song_with_multi_album("album_a/album_b", "2020", 1)];

            pane.stack.insert(Path::new(), vec![DirOrSong::name_only("artist".to_string())]);
            pane.process_songs("artist".to_string(), songs, &ctx);
            pane.stack_mut().current_mut().select_idx(0, 0);

            let albums = pane_albums(&pane);
            assert_eq!(albums, vec!["album_a", "album_b"]);
            let all_songs = pane.songs_for_item(&DirOrSong::name_only("artist".to_string()));
            assert_eq!(all_songs.len(), 2);
        }

        #[rstest]
        fn song_appears_under_each_split_album(mut ctx: Ctx, config: Config) {
            ctx.config = std::sync::Arc::new(config);
            let mut pane = TagBrowserPane::new(
                vec![tag("artist"), BrowserTagConfig {
                    tag: "album".to_string(),
                    split_by_tag: None,
                    sort_by: None,
                    separator: Some("/".to_string()),
                }],
                PaneType::Artists,
                &ctx,
            );
            let song1 = song_with_multi_album("album_a/album_b", "2020", 1);
            let song2 = song_with_multi_album("album_a/album_b", "2021", 2);

            pane.stack.insert(Path::new(), vec![DirOrSong::name_only("artist".to_string())]);
            pane.process_songs("artist".to_string(), vec![song1.clone(), song2.clone()], &ctx);
            pane.stack_mut().current_mut().select_idx(0, 0);
            pane.stack_mut().enter();

            let under_album_a = pane.songs_for_item(&DirOrSong::name_only("album_a".to_string()));
            assert_eq!(under_album_a.len(), 2);

            let under_album_b = pane.songs_for_item(&DirOrSong::name_only("album_b".to_string()));
            assert_eq!(under_album_b.len(), 2);
        }

        #[rstest]
        fn deduplicates_identical_split_values(mut ctx: Ctx, config: Config) {
            ctx.config = std::sync::Arc::new(config);
            let mut pane = TagBrowserPane::new(
                vec![tag("artist"), BrowserTagConfig {
                    tag: "album".to_string(),
                    split_by_tag: None,
                    sort_by: None,
                    separator: Some("/".to_string()),
                }],
                PaneType::Artists,
                &ctx,
            );
            let songs = vec![song_with_multi_album("album_a/album_a", "2020", 1)];

            pane.stack.insert(Path::new(), vec![DirOrSong::name_only("artist".to_string())]);
            pane.process_songs("artist".to_string(), songs, &ctx);
            pane.stack_mut().current_mut().select_idx(0, 0);

            let albums = pane_albums(&pane);
            assert_eq!(albums, vec!["album_a"]);
            let all_songs = pane.songs_for_item(&DirOrSong::name_only("artist".to_string()));
            assert_eq!(all_songs.len(), 1);
        }
    }
}
