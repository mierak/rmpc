use std::borrow::Cow;

#[cfg(debug_assertions)]
use self::{frame_count::FrameCountPane, logs::LogsPane};
use album_art::AlbumArtPane;
use albums::AlbumsPane;
use anyhow::Result;
use artists::{ArtistsPane, ArtistsPaneMode};
use directories::DirectoriesPane;
use either::Either;
use header::HeaderPane;
use lyrics::LyricsPane;
use playlists::PlaylistsPane;
use progress_bar::ProgressBarPane;
use queue::QueuePane;
use ratatui::{
    layout::{Constraint, Layout},
    prelude::Rect,
    text::{Line, Span},
    widgets::Block,
    Frame,
};
use search::SearchPane;
use strum::Display;
use tabs::TabsPane;

use crate::{
    config::{
        keys::CommonAction,
        tabs::{Pane as ConfigPane, PaneType, SizedPaneOrSplit},
        theme::{
            properties::{Property, PropertyKind, PropertyKindOrText, SongProperty, StatusProperty, WidgetProperty},
            SymbolsConfig,
        },
    },
    context::AppContext,
    mpd::commands::{status::OnOffOneshot, volume::Bound, Song, Status},
    shared::{ext::duration::DurationExt, key_event::KeyEvent, mouse_event::MouseEvent},
    MpdQueryResult,
};

use super::{widgets::volume::Volume, UiEvent};

pub mod album_art;
pub mod albums;
pub mod artists;
pub mod directories;
#[cfg(debug_assertions)]
pub mod frame_count;
pub mod header;
#[cfg(debug_assertions)]
pub mod logs;
pub mod lyrics;
pub mod playlists;
pub mod progress_bar;
pub mod queue;
pub mod search;
pub mod tabs;

#[derive(Debug, Display, strum::EnumDiscriminants)]
pub enum Panes<'pane_ref, 'pane> {
    Queue(&'pane_ref mut QueuePane),
    #[cfg(debug_assertions)]
    Logs(&'pane_ref mut LogsPane),
    Directories(&'pane_ref mut DirectoriesPane),
    Artists(&'pane_ref mut ArtistsPane),
    AlbumArtists(&'pane_ref mut ArtistsPane),
    Albums(&'pane_ref mut AlbumsPane),
    Playlists(&'pane_ref mut PlaylistsPane),
    Search(&'pane_ref mut SearchPane),
    AlbumArt(&'pane_ref mut AlbumArtPane),
    Lyrics(&'pane_ref mut LyricsPane),
    ProgressBar(&'pane_ref mut ProgressBarPane),
    Header(&'pane_ref mut HeaderPane),
    Tabs(&'pane_ref mut TabsPane<'pane>),
    #[cfg(debug_assertions)]
    FrameCount(&'pane_ref mut FrameCountPane),
    TabContent,
}

#[derive(Debug)]
pub struct PaneContainer<'panes> {
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
    pub lyrics: LyricsPane,
    pub progress_bar: ProgressBarPane,
    pub header: HeaderPane,
    pub tabs: TabsPane<'panes>,
    #[cfg(debug_assertions)]
    pub frame_count: FrameCountPane,
}

impl<'panes> PaneContainer<'panes> {
    pub fn new(context: &AppContext) -> Result<Self> {
        Ok(Self {
            queue: QueuePane::new(context),
            #[cfg(debug_assertions)]
            logs: LogsPane::new(),
            directories: DirectoriesPane::new(context),
            albums: AlbumsPane::new(context),
            artists: ArtistsPane::new(ArtistsPaneMode::Artist, context),
            album_artists: ArtistsPane::new(ArtistsPaneMode::AlbumArtist, context),
            playlists: PlaylistsPane::new(context),
            search: SearchPane::new(context),
            album_art: AlbumArtPane::new(context),
            lyrics: LyricsPane::new(context),
            progress_bar: ProgressBarPane::new(),
            header: HeaderPane::new(),
            tabs: TabsPane::new(context)?,
            #[cfg(debug_assertions)]
            frame_count: FrameCountPane::new(),
        })
    }

    pub fn get_mut<'pane_ref>(&'pane_ref mut self, screen: PaneType) -> Panes<'pane_ref, 'panes> {
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
            PaneType::Lyrics => Panes::Lyrics(&mut self.lyrics),
            PaneType::ProgressBar => Panes::ProgressBar(&mut self.progress_bar),
            PaneType::Header => Panes::Header(&mut self.header),
            PaneType::Tabs => Panes::Tabs(&mut self.tabs),
            PaneType::TabContent => Panes::TabContent,
            #[cfg(debug_assertions)]
            PaneType::FrameCount => Panes::FrameCount(&mut self.frame_count),
        }
    }
}

macro_rules! pane_call {
    ($screen:ident, $fn:ident($($param:expr),+)) => {
        match $screen {
            Panes::Queue(ref mut s) => s.$fn($($param),+),
            #[cfg(debug_assertions)]
            Panes::Logs(ref mut s) => s.$fn($($param),+),
            Panes::Directories(ref mut s) => s.$fn($($param),+),
            Panes::Artists(ref mut s) => s.$fn($($param),+),
            Panes::AlbumArtists(ref mut s) => s.$fn($($param),+),
            Panes::Albums(ref mut s) => s.$fn($($param),+),
            Panes::Playlists(ref mut s) => s.$fn($($param),+),
            Panes::Search(ref mut s) => s.$fn($($param),+),
            Panes::AlbumArt(ref mut s) => s.$fn($($param),+),
            Panes::Lyrics(ref mut s) => s.$fn($($param),+),
            Panes::ProgressBar(ref mut s) => s.$fn($($param),+),
            Panes::Header(ref mut s) => s.$fn($($param),+),
            Panes::Tabs(ref mut s) => s.$fn($($param),+),
            Panes::TabContent => Ok(()),
            #[cfg(debug_assertions)]
            Panes::FrameCount(ref mut s) => s.$fn($($param),+),
        }
    }
}
pub(crate) use pane_call;

#[allow(unused_variables)]
pub(super) trait Pane {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> Result<()>;

    /// For any cleanup operations, ran when the screen hides
    fn on_hide(&mut self, context: &AppContext) -> Result<()> {
        Ok(())
    }

    /// For work that needs to be done BEFORE the first render
    fn before_show(&mut self, context: &AppContext) -> Result<()> {
        Ok(())
    }

    /// Used to keep the current state but refresh data
    fn on_event(&mut self, event: &mut UiEvent, is_visible: bool, context: &AppContext) -> Result<()> {
        Ok(())
    }

    fn handle_action(&mut self, event: &mut KeyEvent, context: &mut AppContext) -> Result<()>;

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &AppContext) -> Result<()> {
        Ok(())
    }

    fn on_query_finished(
        &mut self,
        id: &'static str,
        data: MpdQueryResult,
        is_visible: bool,
        context: &AppContext,
    ) -> Result<()> {
        Ok(())
    }

    fn calculate_areas(&mut self, area: Rect, context: &AppContext) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, area: Rect, context: &AppContext) -> Result<()> {
        Ok(())
    }
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
        mpd::commands::{lsinfo::LsInfoEntry, Song},
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

            for (k, v) in self
                .metadata
                .iter()
                .filter(|(key, _)| !["title", "album", "artist", "duration"].contains(&(*key).as_str()))
            {
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
        pub fn name_only(name: String) -> Self {
            DirOrSong::Dir {
                name,
                full_path: String::new(),
            }
        }

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
                (DirOrSong::Dir { name: a, .. }, DirOrSong::Dir { name: b, .. }) => a.cmp(b),
                (DirOrSong::Song(_), DirOrSong::Dir { .. }) => Ordering::Greater,
                (DirOrSong::Dir { .. }, DirOrSong::Song(_)) => Ordering::Less,
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

    impl From<LsInfoEntry> for Option<DirOrSong> {
        fn from(value: LsInfoEntry) -> Self {
            match value {
                LsInfoEntry::Dir(crate::mpd::commands::lsinfo::Dir { path, full_path, .. }) => {
                    Some(DirOrSong::Dir { name: path, full_path })
                }
                LsInfoEntry::File(song) => Some(DirOrSong::Song(song)),
                LsInfoEntry::Playlist(_) => None,
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
                PropertyKindOrText::Sticker(key) => self
                    .stickers
                    .as_ref()
                    .and_then(|stickers| {
                        stickers
                            .get(*key)
                            .map(|value| value.to_lowercase().contains(&filter.to_lowercase()))
                    })
                    .or_else(|| format.default.map(|f| self.matches(&[f], filter))),
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
        symbols: &SymbolsConfig,
    ) -> Option<Line<'song>> {
        format
            .default
            .and_then(|f| self.as_line_ellipsized(f, max_len, symbols))
    }

    pub fn as_line_ellipsized<'song>(
        &'song self,
        format: &'static Property<'static, SongProperty>,
        max_len: usize,
        symbols: &SymbolsConfig,
    ) -> Option<Line<'song>> {
        let style = format.style.unwrap_or_default();
        match &format.kind {
            PropertyKindOrText::Text(value) => {
                Some(Line::styled((*value).ellipsize(max_len, symbols).to_string(), style))
            }
            PropertyKindOrText::Sticker(key) => self
                .stickers
                .as_ref()
                .and_then(|stickers| stickers.get(*key))
                .map(|sticker| Line::styled(sticker.ellipsize(max_len, symbols), style))
                .or_else(|| {
                    format
                        .default
                        .and_then(|format| self.as_line_ellipsized(format, max_len, symbols))
                }),
            PropertyKindOrText::Property(property) => self.format(property).map_or_else(
                || self.default_as_line_ellipsized(format, max_len, symbols),
                |v| Some(Line::styled(v.ellipsize(max_len, symbols).into_owned(), style)),
            ),
            PropertyKindOrText::Group(group) => {
                let mut buf = Line::default();
                for grformat in *group {
                    if let Some(res) = self.as_line_ellipsized(grformat, max_len, symbols) {
                        for span in res.spans {
                            buf.push_span(span);
                        }
                    } else {
                        return format
                            .default
                            .and_then(|format| self.as_line_ellipsized(format, max_len, symbols));
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
            PropertyKindOrText::Sticker(key) => {
                if let Some(sticker) = song.map(|s| s.stickers.as_ref().and_then(|stickers| stickers.get(*key))) {
                    sticker.cloned()
                } else {
                    self.default(song)
                }
            }
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
            PropertyKindOrText::Sticker(key) => {
                if let Some(sticker) = song.and_then(|s| s.stickers.as_ref().and_then(|stickers| stickers.get(*key))) {
                    Some(Either::Left(Span::styled(sticker, style)))
                } else {
                    self.default_as_span(song, status)
                }
            }
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
                    |v| Some(Either::Left(Span::styled(v.to_string(), style))),
                ),
                StatusProperty::Crossfade => status.xfade.as_ref().map_or_else(
                    || self.default_as_span(song, status),
                    |v| Some(Either::Left(Span::styled(v.to_string(), style))),
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

impl SizedPaneOrSplit {
    pub fn for_each_pane(
        &self,
        focused: Option<ConfigPane>,
        area: Rect,
        context: &AppContext,
        callback: &mut impl FnMut(ConfigPane, Rect, Block, Rect) -> Result<()>,
    ) -> Result<()> {
        let mut stack = vec![(self, area)];

        while let Some((configured_panes, area)) = stack.pop() {
            match configured_panes {
                SizedPaneOrSplit::Pane(pane) => {
                    let block = Block::default()
                        .border_style(if focused.is_some_and(|p| p.id == pane.id) {
                            context.config.as_focused_border_style()
                        } else {
                            context.config.as_border_style()
                        })
                        .borders(pane.border);
                    let pane_area = block.inner(area);

                    callback(*pane, pane_area, block, area)?;
                }
                SizedPaneOrSplit::Split { direction, panes } => {
                    let constraints = panes.iter().map(|pane| Into::<Constraint>::into(pane.size));
                    let areas = Layout::new(*direction, constraints).split(area);
                    stack.extend(areas.iter().enumerate().map(|(idx, area)| (&panes[idx].pane, *area)));
                }
            }
        }

        Ok(())
    }
}

pub(crate) trait StringExt {
    fn ellipsize(&self, max_len: usize, symbols: &SymbolsConfig) -> Cow<str>;
}

impl StringExt for Cow<'_, str> {
    fn ellipsize(&self, max_len: usize, symbols: &SymbolsConfig) -> Cow<str> {
        if self.chars().count() > max_len {
            Cow::Owned(format!(
                "{}{}",
                self.chars()
                    .take(max_len.saturating_sub(symbols.ellipsis.chars().count()))
                    .collect::<String>(),
                symbols.ellipsis,
            ))
        } else {
            Cow::Borrowed(self)
        }
    }
}

impl StringExt for &str {
    fn ellipsize(&self, max_len: usize, symbols: &SymbolsConfig) -> Cow<str> {
        if self.chars().count() > max_len {
            Cow::Owned(format!(
                "{}{}",
                self.chars()
                    .take(max_len.saturating_sub(symbols.ellipsis.chars().count()))
                    .collect::<String>(),
                symbols.ellipsis,
            ))
        } else {
            Cow::Borrowed(self)
        }
    }
}

impl StringExt for String {
    fn ellipsize(&self, max_len: usize, symbols: &SymbolsConfig) -> Cow<str> {
        if self.chars().count() > max_len {
            Cow::Owned(format!(
                "{}{}",
                self.chars()
                    .take(max_len.saturating_sub(symbols.ellipsis.chars().count()))
                    .collect::<String>(),
                symbols.ellipsis,
            ))
        } else {
            Cow::Borrowed(self)
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
                stickers: None,
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
                stickers: None,
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
