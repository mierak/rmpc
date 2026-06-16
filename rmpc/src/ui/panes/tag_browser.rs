use anyhow::{Context, Result};
use enum_map::EnumMap;
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect, widgets::ListState};
use rmpc_mpd::{
    client::Client,
    commands::Song,
    filter::{Filter, Tag},
    mpd_client::MpdClient,
};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::{
        album_art::ImageMethod,
        sort_mode::{SortMode, SortOptions},
        tabs::{BrowserTagConfig, CollapseLevel, PaneType},
        theme::properties::SongProperty,
    },
    ctx::Ctx,
    shared::{
        cmp::StringCompare,
        id,
        keys::ActionEvent,
        mouse_event::MouseEvent,
        mpd_client_ext::MpdClientExt,
    },
    ui::{
        UiEvent,
        browser::BrowserPane,
        dir_or_song::DirOrSong,
        dirstack::{DirStack, DirState, DirStackItem, Path, WalkDirStackItem},
        image::facade::AlbumArtFacade,
        input::InputResultEvent,
        song_ext::SongExt,
        widgets::browser::{Browser, BrowserArea},
    },
};

#[derive(Debug)]
pub struct TagBrowserPane {
    stack: DirStack<DirOrSong, ListState>,
    tags: Vec<BrowserTagConfig>,
    target_pane: PaneType,
    pub(crate) browser: Browser<DirOrSong>,
    initialized: bool,
    album_art: AlbumArtFacade,
    crisp: bool,
    has_cover: bool,
    shown_cover: Option<String>,
}

const INIT: &str = "init";
const FETCH_SONGS: &str = "fetch_songs";
const PREVIEW_COVER: &str = "tag_browser_preview_cover";

struct SongGroup {
    id: String,
    tags: Vec<Option<String>>,
    display_name: String,
    songs: Vec<Song>,
}

impl TagBrowserPane {
    pub fn new(tags: Vec<BrowserTagConfig>, target_pane: PaneType, ctx: &Ctx) -> Self {
        let crisp = matches!(
            ctx.config.album_art.method,
            ImageMethod::Kitty
                | ImageMethod::Iterm2
                | ImageMethod::Sixel
                | ImageMethod::UeberzugWayland
                | ImageMethod::UeberzugX11
        );
        let mut browser = Browser::new();
        browser.preview_cover = crisp;
        Self {
            tags,
            target_pane,
            stack: DirStack::default(),
            browser,
            initialized: false,
            album_art: AlbumArtFacade::new(ctx),
            crisp,
            has_cover: false,
            shown_cover: None,
        }
    }

    fn group_songs_by_tag(songs: Vec<Song>, tag: &BrowserTagConfig, ctx: &Ctx) -> Vec<SongGroup> {
        let sep = ctx.config.theme.format_tag_separator.as_str();
        let sort_opts = &ctx.config.browser_song_sort;

        let tag_of_with_fallback = |song: &Song, tags: &[SongProperty]| {
            tags.iter()
                .find_map(|tag| {
                    SongExt::format(
                        song,
                        tag,
                        sep,
                        ctx.config.theme.multiple_tag_resolution_strategy,
                    )
                })
                .map(|v| v.into_owned())
        };

        let tags_of = |song: &Song, tags: &Vec<Vec<SongProperty>>| -> Vec<Option<String>> {
            tags.iter().map(|group_tag| tag_of_with_fallback(song, group_tag.as_slice())).collect()
        };

        let groups: Vec<(Vec<Option<String>>, Vec<Song>)> = songs
            .into_iter()
            .into_group_map_by(|s| tags_of(s, &tag.group_by))
            .into_iter()
            .map(|(tags, mut songs)| {
                songs.sort_by(|a, b| {
                    a.with_custom_sort(sort_opts).cmp(&b.with_custom_sort(sort_opts))
                });

                (tags, songs)
            })
            .collect_vec();

        let props = match &tag.sort_by {
            Some(sort_tags) => sort_tags.iter().flat_map(|tag| tag.clone()).collect_vec(),
            None => vec![],
        };
        let opts = SortOptions {
            mode: SortMode::Format(props),
            group_by_type: ctx.config.directories_sort.group_by_type,
            reverse: ctx.config.directories_sort.reverse,
            ignore_leading_the: ctx.config.directories_sort.ignore_leading_the,
            fold_case: ctx.config.directories_sort.fold_case,
        };

        groups
            .into_iter()
            .map(|(tags, songs)| {
                let display_name = DirStackItem::format(&songs[0], &tag.format, "", ctx);
                SongGroup { id: id::new().to_string(), tags, display_name, songs }
            })
            .sorted_by(|a, b| {
                //
                match &tag.sort_by {
                    Some(_) => {
                        // Pick tags from the first song in the group. This is not a perfect
                        // solution in case there are songs with different
                        // tag values within the same group, but
                        // it's hard to come up with a better solution and it should
                        // work well in most cases. Only time this is an issue is when
                        // sort tags are not a subset of the grouping tags.
                        a.songs[0].with_custom_sort(&opts).cmp(&b.songs[0].with_custom_sort(&opts))
                    }
                    None => StringCompare::from(&opts).compare(&a.display_name, &b.display_name),
                }
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
            stack.insert_or_append(current_path, songs.into_iter().map(DirOrSong::Song).collect());
            return;
        }

        let tag = &remaining_tags[0];
        let rest = &remaining_tags[1..];

        let mut groups = Self::group_songs_by_tag(songs, tag, ctx);

        match (&tag.skip, &mut groups[..]) {
            (CollapseLevel::Single, [group]) => {
                Self::insert_level(
                    stack,
                    current_path,
                    std::mem::take(&mut group.songs),
                    rest,
                    ctx,
                );
                return;
            }
            (CollapseLevel::SingleEmpty, [group]) if group.tags.iter().all(|tag| tag.is_none()) => {
                Self::insert_level(
                    stack,
                    current_path,
                    std::mem::take(&mut group.songs),
                    rest,
                    ctx,
                );
                return;
            }
            (CollapseLevel::UnpackEmpty, _) if rest.is_empty() => {
                let (empty, non_empty): (Vec<SongGroup>, Vec<SongGroup>) = groups
                    .into_iter()
                    .partition(|group| group.tags.iter().all(|tag| tag.is_none()));
                groups = non_empty;

                for group in empty {
                    Self::insert_level(stack, current_path.clone(), group.songs, rest, ctx);
                }
            }
            _ => {}
        }

        stack.insert_or_append(
            current_path.clone(),
            groups
                .iter()
                .map(|gr| DirOrSong::name_display_name_only(gr.id.clone(), gr.display_name.clone()))
                .collect(),
        );

        if matches!(tag.skip, CollapseLevel::UnpackEmpty) {
            let dir = stack.get_ensure(current_path.clone());
            dir.items.sort_by_key(|item| matches!(item, DirOrSong::Song(_)));
        }

        for SongGroup { id, tags: _, display_name: _, songs } in groups {
            let child_path = current_path.join(&id);
            Self::insert_level(stack, child_path, songs, rest, ctx);
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

    fn fetch_selected_cover(&mut self, ctx: &Ctx) {
        if !self.crisp {
            return;
        }
        let Some(item) = self.stack.current().selected() else {
            return;
        };
        let key = match item {
            DirOrSong::Song(song) => song.file.clone(),
            DirOrSong::Dir { display_name, name, .. } => {
                display_name.as_ref().unwrap_or(name).clone()
            }
        };
        if self.shown_cover.as_ref() == Some(&key) {
            return;
        }
        self.shown_cover = Some(key.clone());
        let order = ctx.config.album_art.order;
        let item = item.clone();
        let target = self.target_pane.clone();
        ctx.query().id(PREVIEW_COVER).replace_id(PREVIEW_COVER).target(target).query(
            move |client| {
                let cover = match &item {
                    DirOrSong::Song(song) => client.find_album_art(&song.file, order)?,
                    DirOrSong::Dir { display_name, name, .. } => {
                        let album = display_name.as_ref().unwrap_or(name).as_str();
                        match client.find_one(&[Filter::new(Tag::Album, album)])? {
                            Some(song) => client.find_album_art(&song.file, order)?,
                            None => None,
                        }
                    }
                };
                Ok(MpdQueryResult::AlbumArt(cover))
            },
        );
    }
}

impl Pane for TagBrowserPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        self.browser.render(area, frame.buffer_mut(), &mut self.stack, ctx);
        if self.crisp {
            let cover_rect = self.browser.areas[BrowserArea::Cover];
            if cover_rect.width > 0 && cover_rect.height > 0 {
                self.album_art.set_size(cover_rect);
            }
        }
        self.fetch_selected_cover(ctx);
        Ok(())
    }

    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        if !self.initialized {
            let root_tag = self.tags[0].group_by[0][0].clone().try_into()?;
            let target = self.target_pane.clone();
            ctx.query().id(INIT).replace_id(INIT).target(target).query(move |client| {
                let result = client.list_tag(root_tag, None).context("Cannot list artists")?;
                log::debug!("Fetched root tag values: {:?}", result.0);
                Ok(MpdQueryResult::LsInfo { data: result.0, path: None })
            });

            self.initialized = true;
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::Database => {
                let root_tag = self.tags[0].group_by[0][0].clone().try_into()?;
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
            UiEvent::ImageEncoded { id, data } if *id == self.album_art.id() => {
                self.album_art.display(std::mem::take(data), ctx)?;
            }
            UiEvent::ImageEncodeFailed { id, err } if *id == self.album_art.id() => {
                self.album_art.image_processing_failed(err, ctx)?;
            }
            UiEvent::Displayed if is_visible && self.crisp && self.has_cover => {
                self.album_art.show_current(ctx)?;
            }
            UiEvent::Exit => {
                self.album_art.cleanup()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_hide(&mut self, ctx: &Ctx) -> Result<()> {
        self.shown_cover = None;
        self.has_cover = false;
        self.album_art.hide(ctx)
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

                let data = data
                    .into_iter()
                    .sorted_by(|a, b| StringCompare::from(sort_opts).compare(a, b))
                    .map(DirOrSong::name_only)
                    .collect_vec();

                self.stack = DirStack::new(data);
                if let Some(sel) = self.stack.current().selected() {
                    self.fetch_data(sel, ctx)?;
                }
                ctx.render()?;
            }
            (PREVIEW_COVER, MpdQueryResult::AlbumArt(Some(bytes))) => {
                self.has_cover = true;
                self.album_art.show(bytes, ctx)?;
            }
            (PREVIEW_COVER, MpdQueryResult::AlbumArt(None)) => {
                self.has_cover = false;
                self.album_art.hide(ctx)?;
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

    fn preview_scroll_mut(&mut self) -> Option<&mut DirState<ListState>> {
        Some(&mut self.browser.preview_scroll)
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
                let root_tag: Tag = self.tags[0].group_by[0][0].clone().try_into()?;
                let target = self.target_pane.clone();
                let current = current.to_owned();

                ctx.query().id(FETCH_SONGS).replace_id(FETCH_SONGS).target(target).query(
                    move |client| {
                        let all_songs: Vec<Song> =
                            client.find(&[Filter::new(root_tag, &current)])?;
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
        config::{
            Config,
            tabs::CollapseLevel,
            theme::properties::{Property, PropertyKindOrText, SongProperty},
        },
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
            .map(|item| match item {
                DirOrSong::Dir { display_name, .. } => {
                    display_name.as_ref().map_or_else(|| item.as_path().to_string(), |n| n.clone())
                }
                DirOrSong::Song(_) => {
                    panic!(
                        "expected only dirs at this level, found song with path {:?}",
                        item.as_path()
                    );
                }
            })
            .collect_vec()
    }

    fn tag(tag: impl Into<String> + Clone) -> BrowserTagConfig {
        BrowserTagConfig {
            group_by: vec![vec![SongProperty::Other(tag.clone().into())], vec![
                SongProperty::Other("date".to_string()),
            ]],
            sort_by: Some(vec![vec![SongProperty::Other("date".to_string())]]),
            format: vec![
                Property {
                    kind: PropertyKindOrText::Property(SongProperty::Other(tag.into())),
                    style: None,
                    default: None,
                },
                Property {
                    kind: PropertyKindOrText::Text(" ".to_string()),
                    style: None,
                    default: None,
                },
                Property {
                    kind: PropertyKindOrText::Property(SongProperty::Other("date".to_string())),
                    style: None,
                    default: None,
                },
            ],
            skip: CollapseLevel::default(),
        }
    }

    fn album_tag(sort_tag: Option<&str>, date_tags: Option<&[&str]>) -> BrowserTagConfig {
        album_tags(sort_tag.map(|t| vec![t.to_string()]), date_tags)
    }

    fn album_tags(sort_tag: Option<Vec<String>>, date_tags: Option<&[&str]>) -> BrowserTagConfig {
        let mut group_by = vec![vec![SongProperty::Album]];
        if let Some(date_tags) = date_tags {
            group_by.extend(vec![
                date_tags.iter().map(|s| SongProperty::Other(s.to_string())).collect_vec(),
            ]);
        }

        let mut format = vec![Property {
            kind: PropertyKindOrText::Property(SongProperty::Album),
            style: None,
            default: None,
        }];

        if let Some(date_tags) = date_tags {
            format.insert(0, Property {
                kind: PropertyKindOrText::Text(") ".to_string()),
                style: None,
                default: None,
            });
            format.insert(0, Property {
                kind: PropertyKindOrText::Property(SongProperty::Other(date_tags[0].to_string())),
                style: None,
                default: Some(Box::new(Property {
                    kind: date_tags.get(1).map_or_else(
                        || PropertyKindOrText::Text(format!("<no {}>", date_tags[0])),
                        |t| PropertyKindOrText::Property(SongProperty::Other(t.to_string())),
                    ),
                    style: None,
                    default: Some(Box::new(Property {
                        kind: PropertyKindOrText::Text("testing ast".to_string()),
                        style: None,
                        default: None,
                    })),
                })),
            });
            format.insert(0, Property {
                kind: PropertyKindOrText::Text("(".to_string()),
                style: None,
                default: None,
            });
        }

        BrowserTagConfig {
            group_by,
            sort_by: sort_tag
                .map(|tags| tags.into_iter().map(|tag| vec![SongProperty::Other(tag)]).collect()),
            format,
            skip: CollapseLevel::default(),
        }
    }

    fn find_dir_by_display_name<'a>(pane: &'a TagBrowserPane, name: &str) -> &'a DirOrSong {
        pane.stack
            .entries()
            .find_map(|(_path, item)| {
                item.items.iter().find(|i| {
                    matches!(i, DirOrSong::Dir { display_name, .. } if
                        display_name.as_ref().is_some_and(|n| n == name))
                })
            })
            .expect("expected to find album_a dir")
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

        assert_eq!(pane.stack.len(), 5);
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

        assert_eq!(pane.stack.len(), 5);
        assert_eq!(pane_albums(&pane), vec!["(2019) album_b", "(2020) album_a", "(2021) album_a"]);
    }

    #[rstest]
    fn albums_split_date_sort_date_and_then_sort_name(mut ctx: Ctx, config: Config) {
        ctx.config = std::sync::Arc::new(config);
        let mut pane = TagBrowserPane::new(
            vec![
                tag("artist"),
                album_tags(Some(vec!["date".to_string(), "album".to_string()]), Some(&["date"])),
            ],
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

        assert_eq!(pane.stack.len(), 6);
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

        assert_eq!(pane.stack.len(), 4);
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

        assert_eq!(pane.stack.len(), 4);
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

            let result = pane.songs_for_item(find_dir_by_display_name(&pane, "album_a"));

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
                group_by: vec![vec![SongProperty::Disc]],
                sort_by: None,
                format: vec![],
                skip: CollapseLevel::default(),
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

            let result = pane.songs_for_item(find_dir_by_display_name(&pane, "album_a"));

            let expected_count = disc1_songs.len() + disc2_songs.len();
            assert_eq!(result.len(), expected_count);
            assert!(result.iter().all(|s| s.file.contains("album_a")));
        }
    }
}
