use std::borrow::Cow;

use album_art::AlbumArtPane;
use albums::AlbumsPane;
use anyhow::{Context, Result};
use artists::{ArtistsPane, ArtistsPaneMode};
use crossterm::event::{KeyCode, KeyEvent};
use directories::DirectoriesPane;
use either::Either;
use itertools::Itertools;
#[cfg(debug_assertions)]
use logs::LogsPane;
use playlists::PlaylistsPane;
use queue::QueuePane;
use ratatui::{
    prelude::Rect,
    style::Style,
    text::{Line, Span},
    widgets::ListItem,
    Frame,
};
use search::SearchPane;
use strum::{Display, EnumDiscriminants, EnumIter, VariantNames};

use crate::{
    cli::{create_env, run_external},
    config::{
        keys::{CommonAction, GlobalAction},
        tabs::PaneType,
        theme::properties::{Property, PropertyKind, PropertyKindOrText, SongProperty, StatusProperty, WidgetProperty},
        Config,
    },
    context::AppContext,
    mpd::{
        commands::{status::OnOffOneshot, volume::Bound, Song, Status},
        mpd_client::MpdClient,
    },
    utils::DurationExt,
};

use super::{
    utils::dirstack::{DirStack, DirStackItem},
    widgets::volume::Volume,
    KeyHandleResultInternal, UiEvent,
};

pub mod album_art;
pub mod albums;
pub mod artists;
pub mod directories;
#[cfg(debug_assertions)]
pub mod logs;
pub mod playlists;
pub mod queue;
pub mod search;

#[derive(Debug, Display, EnumDiscriminants, VariantNames)]
#[strum_discriminants(derive(VariantNames, EnumIter))]
pub enum Panes<'a> {
    Queue(&'a mut QueuePane),
    #[cfg(debug_assertions)]
    Logs(&'a mut LogsPane),
    Directories(&'a mut DirectoriesPane),
    Artists(&'a mut ArtistsPane),
    AlbumArtists(&'a mut ArtistsPane),
    Albums(&'a mut AlbumsPane),
    Playlists(&'a mut PlaylistsPane),
    Search(&'a mut SearchPane),
    AlbumArt(&'a mut AlbumArtPane),
}

#[derive(Debug)]
pub struct PaneContainer {
    pub queue: QueuePane,
    #[cfg(debug_assertions)]
    pub logs: LogsPane,
    pub directories: DirectoriesPane,
    pub albums: AlbumsPane,
    pub artists: ArtistsPane,
    pub album_artists: ArtistsPane,
    pub playlists: PlaylistsPane,
    pub search: SearchPane,
    pub album_art: AlbumArtPane,
}

impl PaneContainer {
    pub fn new(context: &AppContext) -> Self {
        Self {
            queue: QueuePane::new(context),
            #[cfg(debug_assertions)]
            logs: LogsPane::default(),
            directories: DirectoriesPane::default(),
            albums: AlbumsPane::default(),
            artists: ArtistsPane::new(ArtistsPaneMode::Artist),
            album_artists: ArtistsPane::new(ArtistsPaneMode::AlbumArtist),
            playlists: PlaylistsPane::default(),
            search: SearchPane::new(context.config),
            album_art: AlbumArtPane::new(context),
        }
    }

    pub fn get_mut(&mut self, screen: PaneType) -> Panes {
        match screen {
            PaneType::Queue => Panes::Queue(&mut self.queue),
            #[cfg(debug_assertions)]
            PaneType::Logs => Panes::Logs(&mut self.logs),
            PaneType::Directories => Panes::Directories(&mut self.directories),
            PaneType::Artists => Panes::Artists(&mut self.artists),
            PaneType::AlbumArtists => Panes::AlbumArtists(&mut self.album_artists),
            PaneType::Albums => Panes::Albums(&mut self.albums),
            PaneType::Playlists => Panes::Playlists(&mut self.playlists),
            PaneType::Search => Panes::Search(&mut self.search),
            PaneType::AlbumArt => Panes::AlbumArt(&mut self.album_art),
        }
    }
}

type KeyResult = Result<KeyHandleResultInternal>;
#[allow(unused_variables)]
pub(super) trait Pane {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> Result<()>;
    fn post_render(&mut self, frame: &mut Frame, context: &AppContext) -> Result<()> {
        Ok(())
    }

    /// For any cleanup operations, ran when the screen hides
    fn on_hide(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        Ok(())
    }

    /// For work that needs to be done BEFORE the first render
    fn before_show(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        Ok(())
    }

    /// Used to keep the current state but refresh data
    fn on_event(&mut self, event: &mut UiEvent, client: &mut impl MpdClient, context: &AppContext) -> KeyResult {
        Ok(KeyHandleResultInternal::SkipRender)
    }

    fn handle_action(&mut self, event: KeyEvent, client: &mut impl MpdClient, context: &AppContext) -> KeyResult;
}

pub mod dirstack {}

pub(crate) mod browser {
    use std::{borrow::Cow, cmp::Ordering};

    use ratatui::{
        style::{Color, Style},
        text::{Line, Span},
        widgets::ListItem,
    };

    use crate::{
        config::theme::SymbolsConfig,
        mpd::commands::{lsinfo::FileOrDir, Song},
    };

    impl Song {
        pub(crate) fn to_preview(&self, _symbols: &SymbolsConfig) -> impl Iterator<Item = ListItem<'static>> {
            let key_style = Style::default().fg(Color::Yellow);
            let separator = Span::from(": ");
            let start_of_line_spacer = Span::from(" ");

            let file = Line::from(vec![
                start_of_line_spacer.clone(),
                Span::styled("File", key_style),
                separator.clone(),
                Span::from(self.file.clone()),
            ]);
            let mut r = vec![file];

            if let Some(file_name) = self.file_name() {
                r.push(Line::from(vec![
                    start_of_line_spacer.clone(),
                    Span::styled("Filename", key_style),
                    separator.clone(),
                    Span::from(file_name.into_owned()),
                ]));
            }

            if let Some(title) = self.title() {
                r.push(Line::from(vec![
                    start_of_line_spacer.clone(),
                    Span::styled("Title", key_style),
                    separator.clone(),
                    Span::from(title.clone()),
                ]));
            }
            if let Some(artist) = self.artist() {
                r.push(Line::from(vec![
                    start_of_line_spacer.clone(),
                    Span::styled("Artist", key_style),
                    separator.clone(),
                    Span::from(artist.clone()),
                ]));
            }

            if let Some(album) = self.album() {
                r.push(Line::from(vec![
                    start_of_line_spacer.clone(),
                    Span::styled("Album", key_style),
                    separator.clone(),
                    Span::from(album.clone()),
                ]));
            }

            if let Some(duration) = &self.duration {
                r.push(Line::from(vec![
                    start_of_line_spacer.clone(),
                    Span::styled("Duration", key_style),
                    separator.clone(),
                    Span::from(duration.as_secs().to_string()),
                ]));
            }

            for (k, v) in &self.metadata {
                r.push(Line::from(vec![
                    start_of_line_spacer.clone(),
                    Span::styled(k.clone(), key_style),
                    separator.clone(),
                    Span::from(v.clone()),
                ]));
            }

            r.into_iter().map(ListItem::new)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum DirOrSong {
        Dir { name: String, full_path: String },
        Song(Song),
    }

    impl DirOrSong {
        pub fn dir_name_or_file_name(&self) -> Cow<str> {
            match self {
                DirOrSong::Dir { name, full_path: _ } => Cow::Borrowed(name),
                DirOrSong::Song(song) => Cow::Borrowed(&song.file),
            }
        }
    }

    impl std::cmp::Ord for DirOrSong {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            match (self, other) {
                (_, DirOrSong::Dir { .. }) => Ordering::Greater,
                (DirOrSong::Dir { .. }, _) => Ordering::Less,
                (DirOrSong::Song(a), DirOrSong::Song(b)) => a.cmp(b),
            }
        }
    }

    impl std::cmp::PartialOrd for DirOrSong {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    impl std::cmp::Ord for Song {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            let a_track = self.metadata.get("track").map(|v| v.parse::<u32>());
            let b_track = other.metadata.get("track").map(|v| v.parse::<u32>());
            match (a_track, b_track) {
                (Some(Ok(a)), Some(Ok(b))) => a.cmp(&b),
                (_, Some(Ok(_))) => Ordering::Greater,
                (Some(Ok(_)), _) => Ordering::Less,
                _ => self.title().cmp(&other.title()),
            }
        }
    }

    impl std::cmp::PartialOrd for Song {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    impl From<FileOrDir> for DirOrSong {
        fn from(value: FileOrDir) -> Self {
            match value {
                FileOrDir::Dir(crate::mpd::commands::lsinfo::Dir { path, full_path, .. }) => {
                    DirOrSong::Dir { name: path, full_path }
                }
                FileOrDir::File(song) => DirOrSong::Song(song),
            }
        }
    }

    #[cfg(test)]
    mod test {
        use std::collections::HashMap;

        use crate::mpd::commands::Song;

        use super::DirOrSong;

        fn song(title: &str, track: Option<&str>) -> Song {
            Song {
                metadata: HashMap::from([
                    ("title".to_owned(), title.to_owned()),
                    track.map(|v| ("track".to_owned(), v.to_owned())).into_iter().collect(),
                ]),
                ..Default::default()
            }
        }

        #[test]
        fn dir_before_song() {
            let mut input = vec![
                DirOrSong::Song(Song::default()),
                DirOrSong::Dir {
                    name: "a".to_owned(),
                    full_path: String::new(),
                },
                DirOrSong::Song(Song::default()),
                DirOrSong::Dir {
                    name: "z".to_owned(),
                    full_path: String::new(),
                },
                DirOrSong::Song(Song::default()),
            ];

            input.sort();

            assert_eq!(
                input,
                vec![
                    DirOrSong::Dir {
                        name: "a".to_owned(),
                        full_path: String::new()
                    },
                    DirOrSong::Dir {
                        name: "z".to_owned(),
                        full_path: String::new()
                    },
                    DirOrSong::Song(Song::default()),
                    DirOrSong::Song(Song::default()),
                    DirOrSong::Song(Song::default()),
                ]
            );
        }

        #[test]
        fn all_by_track() {
            let mut input = vec![
                DirOrSong::Song(song("a", Some("8"))),
                DirOrSong::Dir {
                    name: "a".to_owned(),
                    full_path: String::new(),
                },
                DirOrSong::Song(song("b", Some("3"))),
                DirOrSong::Dir {
                    name: "z".to_owned(),
                    full_path: String::new(),
                },
                DirOrSong::Song(song("c", Some("5"))),
            ];

            input.sort();

            assert_eq!(
                input,
                vec![
                    DirOrSong::Dir {
                        name: "a".to_owned(),
                        full_path: String::new()
                    },
                    DirOrSong::Dir {
                        name: "z".to_owned(),
                        full_path: String::new()
                    },
                    DirOrSong::Song(song("b", Some("3"))),
                    DirOrSong::Song(song("c", Some("5"))),
                    DirOrSong::Song(song("a", Some("8"))),
                ]
            );
        }

        #[test]
        fn by_track_then_title() {
            let mut input = vec![
                DirOrSong::Song(song("d", Some("10"))),
                DirOrSong::Song(song("a", None)),
                DirOrSong::Dir {
                    name: "a".to_owned(),
                    full_path: String::new(),
                },
                DirOrSong::Song(song("b", Some("3"))),
                DirOrSong::Dir {
                    name: "z".to_owned(),
                    full_path: String::new(),
                },
                DirOrSong::Song(song("c", None)),
            ];

            input.sort();

            assert_eq!(
                input,
                vec![
                    DirOrSong::Dir {
                        name: "a".to_owned(),
                        full_path: String::new()
                    },
                    DirOrSong::Dir {
                        name: "z".to_owned(),
                        full_path: String::new()
                    },
                    DirOrSong::Song(song("b", Some("3"))),
                    DirOrSong::Song(song("d", Some("10"))),
                    DirOrSong::Song(song("a", None)),
                    DirOrSong::Song(song("c", None)),
                ]
            );
        }

        #[test]
        fn by_track_then_title_with_unparsable_track() {
            let mut input = vec![
                DirOrSong::Song(song("d", Some("10"))),
                DirOrSong::Song(song("a", Some("lol"))),
                DirOrSong::Dir {
                    name: "a".to_owned(),
                    full_path: String::new(),
                },
                DirOrSong::Song(song("b", Some("3"))),
                DirOrSong::Dir {
                    name: "z".to_owned(),
                    full_path: String::new(),
                },
                DirOrSong::Song(song("c", None)),
            ];

            input.sort();

            assert_eq!(
                input,
                vec![
                    DirOrSong::Dir {
                        name: "a".to_owned(),
                        full_path: String::new()
                    },
                    DirOrSong::Dir {
                        name: "z".to_owned(),
                        full_path: String::new()
                    },
                    DirOrSong::Song(song("b", Some("3"))),
                    DirOrSong::Song(song("d", Some("10"))),
                    DirOrSong::Song(song("a", Some("lol"))),
                    DirOrSong::Song(song("c", None)),
                ]
            );
        }
    }
}

impl Song {
    pub fn title_str(&self) -> &str {
        self.title().map_or("Untitled", |v| v.as_str())
    }

    pub fn artist_str(&self) -> &str {
        self.artist().map_or("Untitled", |v| v.as_str())
    }

    pub fn file_name(&self) -> Option<Cow<str>> {
        std::path::Path::new(&self.file)
            .file_name()
            .map(|file_name| file_name.to_string_lossy())
    }

    fn format<'song>(&'song self, property: &SongProperty) -> Option<Cow<'song, str>> {
        match property {
            SongProperty::Filename => self.file_name(),
            SongProperty::File => Some(Cow::Borrowed(self.file.as_str())),
            SongProperty::Title => self.title().map(|v| Cow::Borrowed(v.as_ref())),
            SongProperty::Artist => self.artist().map(|v| Cow::Borrowed(v.as_ref())),
            SongProperty::Album => self.album().map(|v| Cow::Borrowed(v.as_ref())),
            SongProperty::Track => self
                .metadata
                .get("track")
                .map(|v| Cow::Owned(v.parse::<u32>().map_or_else(|_| v.clone(), |v| format!("{v:0>2}")))),
            SongProperty::Duration => self.duration.map(|d| Cow::Owned(d.to_string())),
            SongProperty::Other(name) => self.metadata.get(*name).map(|v| Cow::Borrowed(v.as_str())),
        }
    }

    pub fn matches(&self, formats: &[&Property<'static, SongProperty>], filter: &str) -> bool {
        for format in formats {
            let match_found = match &format.kind {
                PropertyKindOrText::Text(value) => Some(value.to_lowercase().contains(&filter.to_lowercase())),
                PropertyKindOrText::Property(property) => self.format(property).map_or_else(
                    || format.default.map(|f| self.matches(&[f], filter)),
                    |p| Some(p.to_lowercase().contains(filter)),
                ),
                PropertyKindOrText::Group(_) => format
                    .as_string(Some(self))
                    .map(|v| v.to_lowercase().contains(&filter.to_lowercase())),
            };
            if match_found.is_some_and(|v| v) {
                return true;
            }
        }
        return false;
    }

    fn default_as_line_ellipsized<'song>(
        &'song self,
        format: &'static Property<'static, SongProperty>,
        max_len: usize,
    ) -> Option<Line<'song>> {
        format.default.and_then(|f| self.as_line_ellipsized(f, max_len))
    }

    pub fn as_line_ellipsized<'song>(
        &'song self,
        format: &'static Property<'static, SongProperty>,
        max_len: usize,
    ) -> Option<Line<'song>> {
        let style = format.style.unwrap_or_default();
        match &format.kind {
            PropertyKindOrText::Text(value) => Some(Line::styled((*value).ellipsize(max_len).to_string(), style)),
            PropertyKindOrText::Property(property) => self.format(property).map_or_else(
                || self.default_as_line_ellipsized(format, max_len),
                |v| Some(Line::styled(v.ellipsize(max_len).into_owned(), style)),
            ),
            PropertyKindOrText::Group(group) => {
                let mut buf = Line::default();
                for grformat in *group {
                    if let Some(res) = self.as_line_ellipsized(grformat, max_len) {
                        for span in res.spans {
                            buf.push_span(span);
                        }
                    } else {
                        return format
                            .default
                            .and_then(|format| self.as_line_ellipsized(format, max_len));
                    }
                }
                return Some(buf);
            }
        }
    }
}

impl Property<'static, SongProperty> {
    fn default(&self, song: Option<&Song>) -> Option<String> {
        self.default.and_then(|p| p.as_string(song))
    }

    pub fn as_string(&self, song: Option<&Song>) -> Option<String> {
        match &self.kind {
            PropertyKindOrText::Text(value) => Some((*value).to_string()),
            PropertyKindOrText::Property(property) => {
                if let Some(song) = song {
                    song.format(property)
                        .map_or_else(|| self.default(Some(song)), |v| Some(v.into_owned()))
                } else {
                    self.default(song)
                }
            }
            PropertyKindOrText::Group(group) => {
                let mut buf = String::new();
                for format in *group {
                    if let Some(res) = format.as_string(song) {
                        buf.push_str(&res);
                    } else {
                        return self.default.and_then(|d| d.as_string(song));
                    }
                }
                return Some(buf);
            }
        }
    }
}

impl Property<'static, PropertyKind> {
    fn default_as_span<'song: 's, 's>(
        &self,
        song: Option<&'song Song>,
        status: &'song Status,
    ) -> Option<Either<Span<'s>, Vec<Span<'s>>>> {
        self.default.and_then(|p| p.as_span(song, status))
    }

    pub fn as_span<'song: 's, 's>(
        &'s self,
        song: Option<&'song Song>,
        status: &'song Status,
    ) -> Option<Either<Span<'s>, Vec<Span<'s>>>> {
        let style = self.style.unwrap_or_default();
        match &self.kind {
            PropertyKindOrText::Text(value) => Some(Either::Left(Span::styled(*value, style))),
            PropertyKindOrText::Property(PropertyKind::Song(property)) => {
                if let Some(song) = song {
                    song.format(property).map_or_else(
                        || self.default_as_span(Some(song), status),
                        |s| Some(Either::Left(Span::styled(s, style))),
                    )
                } else {
                    self.default_as_span(song, status)
                }
            }
            PropertyKindOrText::Property(PropertyKind::Status(s)) => match s {
                StatusProperty::State => Some(Either::Left(Span::styled(status.state.as_ref(), style))),
                StatusProperty::Duration => Some(Either::Left(Span::styled(status.duration.to_string(), style))),
                StatusProperty::Elapsed => Some(Either::Left(Span::styled(status.elapsed.to_string(), style))),
                StatusProperty::Volume => Some(Either::Left(Span::styled(status.volume.value().to_string(), style))),
                StatusProperty::Repeat => Some(Either::Left(Span::styled(
                    if status.repeat { "On" } else { "Off" },
                    style,
                ))),
                StatusProperty::Random => Some(Either::Left(Span::styled(
                    if status.random { "On" } else { "Off" },
                    style,
                ))),
                StatusProperty::Consume => Some(Either::Left(Span::styled(status.consume.to_string(), style))),
                StatusProperty::Single => Some(Either::Left(Span::styled(status.single.to_string(), style))),
                StatusProperty::Bitrate => status.bitrate.as_ref().map_or_else(
                    || self.default_as_span(song, status),
                    |v| Some(Either::Left(Span::styled(v.to_string(), Style::default()))),
                ),
                StatusProperty::Crossfade => status.xfade.as_ref().map_or_else(
                    || self.default_as_span(song, status),
                    |v| Some(Either::Left(Span::styled(v.to_string(), Style::default()))),
                ),
            },
            PropertyKindOrText::Property(PropertyKind::Widget(w)) => match w {
                WidgetProperty::Volume => Some(Either::Left(Span::styled(
                    Volume::get_str(*status.volume.value()),
                    style,
                ))),
                WidgetProperty::States {
                    active_style,
                    separator_style,
                } => {
                    let separator = Span::styled(" / ", *separator_style);
                    Some(Either::Right(vec![
                        Span::styled("Repeat", if status.repeat { *active_style } else { style }),
                        separator.clone(),
                        Span::styled("Random", if status.random { *active_style } else { style }),
                        separator.clone(),
                        match status.consume {
                            OnOffOneshot::On => Span::styled("Consume", *active_style),
                            OnOffOneshot::Off => Span::styled("Consume", style),
                            OnOffOneshot::Oneshot => Span::styled("Oneshot(C)", *active_style),
                        },
                        separator,
                        match status.single {
                            OnOffOneshot::On => Span::styled("Single", *active_style),
                            OnOffOneshot::Off => Span::styled("Single", style),
                            OnOffOneshot::Oneshot => Span::styled("Oneshot(S)", *active_style),
                        },
                    ]))
                }
            },
            PropertyKindOrText::Group(group) => {
                let mut buf = Vec::new();
                for format in *group {
                    match format.as_span(song, status) {
                        Some(Either::Left(span)) => buf.push(span),
                        Some(Either::Right(spans)) => buf.extend(spans),
                        None => return None,
                    }
                }
                return Some(Either::Right(buf));
            }
        }
    }
}

pub(crate) trait StringExt {
    fn ellipsize(&self, max_len: usize) -> Cow<str>;
}

impl StringExt for Cow<'_, str> {
    fn ellipsize(&self, max_len: usize) -> Cow<str> {
        if self.chars().count() > max_len {
            Cow::Owned(format!(
                "{}...",
                self.chars().take(max_len.saturating_sub(4)).collect::<String>()
            ))
        } else {
            Cow::Borrowed(self)
        }
    }
}

impl StringExt for &str {
    fn ellipsize(&self, max_len: usize) -> Cow<str> {
        if self.chars().count() > max_len {
            Cow::Owned(format!(
                "{}...",
                self.chars().take(max_len.saturating_sub(4)).collect::<String>()
            ))
        } else {
            Cow::Borrowed(self)
        }
    }
}

impl StringExt for String {
    fn ellipsize(&self, max_len: usize) -> Cow<str> {
        if self.chars().count() > max_len {
            Cow::Owned(format!(
                "{}...",
                self.chars().take(max_len.saturating_sub(4)).collect::<String>()
            ))
        } else {
            Cow::Borrowed(self)
        }
    }
}

enum MoveDirection {
    Up,
    Down,
}

#[allow(unused)]
trait BrowserPane<T: DirStackItem + std::fmt::Debug>: Pane {
    fn stack(&self) -> &DirStack<T>;
    fn stack_mut(&mut self) -> &mut DirStack<T>;
    fn set_filter_input_mode_active(&mut self, active: bool);
    fn is_filter_input_mode_active(&self) -> bool;
    fn next(&mut self, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal>;
    fn list_songs_in_item(&self, client: &mut impl MpdClient, item: &T) -> Result<Vec<Song>>;
    fn move_selected(
        &mut self,
        direction: MoveDirection,
        client: &mut impl MpdClient,
    ) -> Result<KeyHandleResultInternal> {
        Ok(KeyHandleResultInternal::SkipRender)
    }
    fn prepare_preview(
        &mut self,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<Option<Vec<ListItem<'static>>>>;
    fn add(&self, item: &T, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal>;
    fn add_all(&self, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal>;
    fn delete(&self, item: &T, index: usize, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        Ok(KeyHandleResultInternal::SkipRender)
    }
    fn rename(&self, item: &T, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        Ok(KeyHandleResultInternal::SkipRender)
    }
    fn handle_filter_input(
        &mut self,
        event: KeyEvent,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        match config.keybinds.navigation.get(&event.into()) {
            Some(CommonAction::Close) => {
                self.set_filter_input_mode_active(false);
                self.stack_mut().current_mut().filter = None;
                let preview = self.prepare_preview(client, config)?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            Some(CommonAction::Confirm) => {
                self.set_filter_input_mode_active(false);
                self.stack_mut().current_mut().jump_next_matching(config);
                let preview = self.prepare_preview(client, config)?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            _ => match event.code {
                KeyCode::Char(c) => {
                    if let Some(ref mut f) = self.stack_mut().current_mut().filter {
                        f.push(c);
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Backspace => {
                    if let Some(ref mut f) = self.stack_mut().current_mut().filter {
                        f.pop();
                    };
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                _ => Ok(KeyHandleResultInternal::SkipRender),
            },
        }
    }

    fn handle_global_action(
        &mut self,
        action: GlobalAction,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<KeyHandleResultInternal> {
        match action {
            GlobalAction::ExternalCommand { command, .. } if !self.stack().current().marked().is_empty() => {
                let songs: Vec<_> = self
                    .stack()
                    .current()
                    .marked_items()
                    .map(|item| self.list_songs_in_item(client, item))
                    .flatten_ok()
                    .try_collect()?;
                let songs = songs.iter().map(|song| song.file.as_str()).collect_vec();

                run_external(command, create_env(context, songs, client)?);

                Ok(KeyHandleResultInternal::SkipRender)
            }
            GlobalAction::ExternalCommand { command, .. } => {
                if let Some(selected) = self.stack().current().selected() {
                    let songs = self.list_songs_in_item(client, selected)?;
                    let songs = songs.iter().map(|s| s.file.as_str());

                    run_external(command, create_env(context, songs, client)?);
                }
                Ok(KeyHandleResultInternal::SkipRender)
            }
            _ => Ok(KeyHandleResultInternal::KeyNotHandled),
        }
    }

    fn handle_common_action(
        &mut self,
        action: CommonAction,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<KeyHandleResultInternal> {
        let config = context.config;
        match action {
            CommonAction::Up => {
                self.stack_mut().current_mut().prev();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Down => {
                self.stack_mut().current_mut().next();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::MoveUp => {
                let res = self.move_selected(MoveDirection::Up, client)?;
                Ok(res)
            }
            CommonAction::MoveDown => {
                let res = self.move_selected(MoveDirection::Down, client)?;
                Ok(res)
            }
            CommonAction::DownHalf => {
                self.stack_mut().current_mut().next_half_viewport();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::UpHalf => {
                self.stack_mut().current_mut().prev_half_viewport();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Bottom => {
                self.stack_mut().current_mut().last();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Top => {
                self.stack_mut().current_mut().first();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Right => {
                let res = self.next(client)?;
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(res)
            }
            CommonAction::Left => {
                self.stack_mut().pop();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::EnterSearch => {
                self.set_filter_input_mode_active(true);
                self.stack_mut().current_mut().filter = Some(String::new());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::NextResult => {
                self.stack_mut().current_mut().jump_next_matching(config);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::PreviousResult => {
                self.stack_mut().current_mut().jump_previous_matching(config);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Select => {
                self.stack_mut().current_mut().toggle_mark_selected();
                self.stack_mut().current_mut().next();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Add if !self.stack().current().marked().is_empty() => {
                for idx in self.stack().current().marked().iter().rev() {
                    let item = &self.stack().current().items[*idx];
                    self.add(item, client)?;
                }
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Add => {
                if let Some(item) = self.stack().current().selected() {
                    self.add(item, client)
                } else {
                    Ok(KeyHandleResultInternal::SkipRender)
                }
            }
            CommonAction::AddAll if !self.stack().current().items.is_empty() => {
                self.add_all(client)?;
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::AddAll => Ok(KeyHandleResultInternal::SkipRender),
            CommonAction::Delete if !self.stack().current().marked().is_empty() => {
                for idx in self.stack().current().marked().iter().rev() {
                    let item = &self.stack().current().items[*idx];
                    self.delete(item, *idx, client)?;
                }
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Delete => {
                if let Some((index, item)) = self.stack().current().selected_with_idx() {
                    self.delete(item, index, client)?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                } else {
                    Ok(KeyHandleResultInternal::SkipRender)
                }
            }
            CommonAction::Rename => {
                if let Some(item) = self.stack().current().selected() {
                    self.rename(item, client)
                } else {
                    Ok(KeyHandleResultInternal::SkipRender)
                }
            }
            CommonAction::FocusInput => Ok(KeyHandleResultInternal::SkipRender),
            CommonAction::Close => Ok(KeyHandleResultInternal::SkipRender), // todo out?
            CommonAction::Confirm => Ok(KeyHandleResultInternal::SkipRender), // todo next?
            CommonAction::PaneDown => Ok(KeyHandleResultInternal::SkipRender),
            CommonAction::PaneUp => Ok(KeyHandleResultInternal::SkipRender),
            CommonAction::PaneRight => Ok(KeyHandleResultInternal::SkipRender),
            CommonAction::PaneLeft => Ok(KeyHandleResultInternal::SkipRender),
        }
    }
}

#[cfg(test)]
mod format_tests {
    use crate::{
        config::{
            theme::properties::{Property, PropertyKindOrText, SongProperty},
            Leak,
        },
        mpd::commands::Song,
    };

    mod correct_values {
        use std::{collections::HashMap, time::Duration};

        use ratatui::text::Span;
        use test_case::test_case;

        use crate::{
            config::theme::properties::{PropertyKind, StatusProperty},
            mpd::commands::{status::OnOffOneshot, State, Status, Volume},
        };

        use super::*;

        #[test_case(SongProperty::Title, "title")]
        #[test_case(SongProperty::Artist, "artist")]
        #[test_case(SongProperty::Album, "album")]
        #[test_case(SongProperty::Track, "123")]
        #[test_case(SongProperty::Duration, "2:03")]
        #[test_case(SongProperty::Other("track"), "123")]
        fn song_property_resolves_correctly(prop: SongProperty, expected: &str) {
            let format = Property::<'static, SongProperty> {
                kind: PropertyKindOrText::Property(prop),
                style: None,
                default: None,
            };

            let song = Song {
                id: 123,
                file: "file".to_owned(),
                duration: Some(Duration::from_secs(123)),
                metadata: HashMap::from([
                    ("title".to_string(), "title".to_owned()),
                    ("album".to_string(), "album".to_owned()),
                    ("track".to_string(), "123".to_string()),
                    ("artist".to_string(), "artist".to_string()),
                ]),
            };

            let result = format.as_string(Some(&song));

            assert_eq!(result, Some(expected.to_string()));
        }

        #[test_case(StatusProperty::Volume, "100")]
        #[test_case(StatusProperty::Repeat, "On")]
        #[test_case(StatusProperty::Random, "On")]
        #[test_case(StatusProperty::Single, "On")]
        #[test_case(StatusProperty::Consume, "On")]
        #[test_case(StatusProperty::Elapsed, "2:03")]
        #[test_case(StatusProperty::Duration, "2:03")]
        #[test_case(StatusProperty::Crossfade, "3")]
        #[test_case(StatusProperty::Bitrate, "123")]
        fn status_property_resolves_correctly(prop: StatusProperty, expected: &str) {
            let format = Property::<'static, PropertyKind> {
                kind: PropertyKindOrText::Property(PropertyKind::Status(prop)),
                style: None,
                default: None,
            };

            let song = Song {
                id: 123,
                file: "file".to_owned(),
                duration: Some(Duration::from_secs(123)),
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".to_string()),
                    ("album".to_string(), "album".to_owned()),
                    ("title".to_string(), "title".to_owned()),
                    ("track".to_string(), "123".to_string()),
                ]),
            };
            let status = Status {
                volume: Volume::new(123),
                repeat: true,
                random: true,
                single: OnOffOneshot::On,
                consume: OnOffOneshot::On,
                bitrate: Some(123),
                elapsed: Duration::from_secs(123),
                duration: Duration::from_secs(123),
                xfade: Some(3),
                state: State::Play,
                ..Default::default()
            };

            let result = format.as_span(Some(&song), &status);

            assert_eq!(
                result,
                Some(either::Either::<Span<'_>, Vec<Span<'_>>>::Left(Span::raw(expected)))
            );
        }
    }

    mod property {
        use std::collections::HashMap;

        use super::*;

        #[test]
        fn works() {
            let format = Property::<'static, SongProperty> {
                kind: PropertyKindOrText::Property(SongProperty::Title),
                style: None,
                default: None,
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".to_string()),
                    ("title".to_string(), "title".to_owned()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song));

            assert_eq!(result, Some("title".to_owned()));
        }

        #[test]
        fn falls_back() {
            let format = Property::<'static, SongProperty> {
                kind: PropertyKindOrText::Property(SongProperty::Track),
                style: None,
                default: Some(
                    Property {
                        kind: PropertyKindOrText::Text("fallback"),
                        style: None,
                        default: None,
                    }
                    .leak(),
                ),
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".to_string()),
                    ("title".to_string(), "title".to_owned()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song));

            assert_eq!(result, Some("fallback".to_owned()));
        }

        #[test]
        fn falls_back_to_none() {
            let format = Property::<'static, SongProperty> {
                kind: PropertyKindOrText::Property(SongProperty::Track),
                style: None,
                default: None,
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".to_string()),
                    ("title".to_string(), "title".to_owned()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song));

            assert_eq!(result, None);
        }
    }

    mod text {
        use std::collections::HashMap;

        use super::*;

        #[test]
        fn works() {
            let format = Property::<'static, SongProperty> {
                kind: PropertyKindOrText::Text("test"),
                style: None,
                default: None,
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".to_string()),
                    ("title".to_string(), "title".to_owned()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song));

            assert_eq!(result, Some("test".to_owned()));
        }

        #[test]
        fn fallback_is_ignored() {
            let format = Property::<'static, SongProperty> {
                kind: PropertyKindOrText::Text("test"),
                style: None,
                default: Some(
                    Property {
                        kind: PropertyKindOrText::Text("fallback"),
                        style: None,
                        default: None,
                    }
                    .leak(),
                ),
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".to_string()),
                    ("title".to_string(), "title".to_owned()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song));

            assert_eq!(result, Some("test".to_owned()));
        }
    }

    mod group {
        use std::collections::HashMap;

        use super::*;

        #[test]
        fn group_no_fallback() {
            let format = Property::<'static, SongProperty> {
                kind: PropertyKindOrText::Group(&[
                    &Property {
                        kind: PropertyKindOrText::Property(SongProperty::Track),
                        style: None,
                        default: None,
                    },
                    &Property {
                        kind: PropertyKindOrText::Text(" "),
                        style: None,
                        default: None,
                    },
                ]),
                style: None,
                default: None,
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".to_string()),
                    ("title".to_string(), "title".to_owned()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song));

            assert_eq!(result, None);
        }

        #[test]
        fn group_fallback() {
            let format = Property::<'static, SongProperty> {
                kind: PropertyKindOrText::Group(&[
                    &Property {
                        kind: PropertyKindOrText::Property(SongProperty::Track),
                        style: None,
                        default: None,
                    },
                    &Property {
                        kind: PropertyKindOrText::Text(" "),
                        style: None,
                        default: None,
                    },
                ]),
                style: None,
                default: Some(
                    Property {
                        kind: PropertyKindOrText::Text("fallback"),
                        style: None,
                        default: None,
                    }
                    .leak(),
                ),
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".to_string()),
                    ("title".to_string(), "title".to_owned()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song));

            assert_eq!(result, Some("fallback".to_owned()));
        }

        #[test]
        fn group_resolved() {
            let format = Property::<'static, SongProperty> {
                kind: PropertyKindOrText::Group(&[
                    &Property {
                        kind: PropertyKindOrText::Property(SongProperty::Title),
                        style: None,
                        default: None,
                    },
                    &Property {
                        kind: PropertyKindOrText::Text("text"),
                        style: None,
                        default: None,
                    },
                ]),
                style: None,
                default: Some(
                    Property {
                        kind: PropertyKindOrText::Text("fallback"),
                        style: None,
                        default: None,
                    }
                    .leak(),
                ),
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".to_string()),
                    ("title".to_string(), "title".to_owned()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song));

            assert_eq!(result, Some("titletext".to_owned()));
        }

        #[test]
        fn group_fallback_in_group() {
            let format = Property::<'static, SongProperty> {
                kind: PropertyKindOrText::Group(&[
                    &Property {
                        kind: PropertyKindOrText::Property(SongProperty::Track),
                        style: None,
                        default: Some(&Property {
                            kind: PropertyKindOrText::Text("fallback"),
                            style: None,
                            default: None,
                        }),
                    },
                    &Property {
                        kind: PropertyKindOrText::Text("text"),
                        style: None,
                        default: None,
                    },
                ]),
                style: None,
                default: None,
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".to_string()),
                    ("title".to_string(), "title".to_owned()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song));

            assert_eq!(result, Some("fallbacktext".to_owned()));
        }

        #[test]
        fn group_nesting() {
            let format = Property::<'static, SongProperty> {
                kind: PropertyKindOrText::Group(&[
                    &Property {
                        kind: PropertyKindOrText::Group(&[
                            &Property {
                                kind: PropertyKindOrText::Property(SongProperty::Track),
                                style: None,
                                default: None,
                            },
                            &Property {
                                kind: PropertyKindOrText::Text("inner"),
                                style: None,
                                default: None,
                            },
                        ]),
                        style: None,
                        default: Some(&Property {
                            kind: PropertyKindOrText::Text("innerfallback"),
                            style: None,
                            default: None,
                        }),
                    },
                    &Property {
                        kind: PropertyKindOrText::Text("outer"),
                        style: None,
                        default: None,
                    },
                ]),
                style: None,
                default: None,
            };

            let song = Song {
                metadata: HashMap::from([("title".to_string(), "title".to_owned())]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song));

            assert_eq!(result, Some("innerfallbackouter".to_owned()));
        }
    }
}
