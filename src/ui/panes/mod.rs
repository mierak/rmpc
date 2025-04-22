use std::{borrow::Cow, cmp::Ordering, collections::HashMap};

use album_art::AlbumArtPane;
use albums::AlbumsPane;
use anyhow::{Context, Result};
use directories::DirectoriesPane;
use either::Either;
use header::HeaderPane;
use lyrics::LyricsPane;
use playlists::PlaylistsPane;
use progress_bar::ProgressBarPane;
use property::PropertyPane;
use queue::QueuePane;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    prelude::Rect,
    text::{Line, Span},
    widgets::Block,
};
use search::SearchPane;
use strum::Display;
use tabs::TabsPane;
use tag_browser::TagBrowserPane;
use unicase::UniCase;

#[cfg(debug_assertions)]
use self::{frame_count::FrameCountPane, logs::LogsPane};
use super::{
    UiEvent,
    widgets::{
        scan_status::{ScanStatus, ScanStatusState},
        volume::Volume,
    },
};
use crate::{
    MpdQueryResult,
    config::{
        keys::CommonAction,
        tabs::{Pane as ConfigPane, PaneType, SizedPaneOrSplit},
        theme::{
            SymbolsConfig,
            properties::{
                Property, PropertyKind, PropertyKindOrText, SongProperty, StatusProperty,
                WidgetProperty,
            },
        },
    },
    context::AppContext,
    mpd::{
        commands::{Song, State, Status, status::OnOffOneshot, volume::Bound},
        mpd_client::Tag,
    },
    shared::{ext::duration::DurationExt, key_event::KeyEvent, mouse_event::MouseEvent},
};

pub mod album_art;
pub mod albums;
pub mod directories;
#[cfg(debug_assertions)]
pub mod frame_count;
pub mod header;
#[cfg(debug_assertions)]
pub mod logs;
pub mod lyrics;
pub mod playlists;
pub mod progress_bar;
pub mod property;
pub mod queue;
pub mod search;
pub mod tabs;
pub mod tag_browser;

#[derive(Debug, Display, strum::EnumDiscriminants)]
pub enum Panes<'pane_ref, 'pane> {
    Queue(&'pane_ref mut QueuePane),
    #[cfg(debug_assertions)]
    Logs(&'pane_ref mut LogsPane),
    Directories(&'pane_ref mut DirectoriesPane),
    Artists(&'pane_ref mut TagBrowserPane),
    AlbumArtists(&'pane_ref mut TagBrowserPane),
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
    Property(PropertyPane<'pane_ref>),
    Others(&'pane_ref mut Box<dyn BoxedPane>),
}

pub trait BoxedPane: Pane + std::fmt::Debug {}

impl<P: Pane + std::fmt::Debug> BoxedPane for P {}

#[derive(Debug)]
pub struct PaneContainer<'panes> {
    pub queue: QueuePane,
    #[cfg(debug_assertions)]
    pub logs: LogsPane,
    pub directories: DirectoriesPane,
    pub albums: AlbumsPane,
    pub artists: TagBrowserPane,
    pub album_artists: TagBrowserPane,
    pub playlists: PlaylistsPane,
    pub search: SearchPane,
    pub album_art: AlbumArtPane,
    pub lyrics: LyricsPane,
    pub progress_bar: ProgressBarPane,
    pub header: HeaderPane,
    pub tabs: TabsPane<'panes>,
    #[cfg(debug_assertions)]
    pub frame_count: FrameCountPane,
    pub others: HashMap<PaneType, Box<dyn BoxedPane>>,
}

impl<'panes> PaneContainer<'panes> {
    pub fn new(context: &AppContext) -> Result<Self> {
        Ok(Self {
            queue: QueuePane::new(context),
            #[cfg(debug_assertions)]
            logs: LogsPane::new(),
            directories: DirectoriesPane::new(context),
            albums: AlbumsPane::new(context),
            artists: TagBrowserPane::new(Tag::Artist, PaneType::Artists, None, context),
            album_artists: TagBrowserPane::new(
                Tag::AlbumArtist,
                PaneType::AlbumArtists,
                None,
                context,
            ),
            playlists: PlaylistsPane::new(context),
            search: SearchPane::new(context),
            album_art: AlbumArtPane::new(context),
            lyrics: LyricsPane::new(context),
            progress_bar: ProgressBarPane::new(),
            header: HeaderPane::new(),
            tabs: TabsPane::new(context)?,
            #[cfg(debug_assertions)]
            frame_count: FrameCountPane::new(),
            others: Self::init_other_panes(context).collect(),
        })
    }

    pub fn init_other_panes(
        context: &AppContext,
    ) -> impl Iterator<Item = (PaneType, Box<dyn BoxedPane>)> + use<'_> {
        context.config.tabs.tabs.iter().flat_map(|(_name, tab)| {
            tab.panes.panes_iter().filter_map(|pane| match &pane.pane {
                PaneType::Browser { root_tag, separator } => Some((
                    pane.pane.clone(),
                    Box::new(TagBrowserPane::new(
                        Tag::Custom(root_tag.clone()),
                        pane.pane.clone(),
                        separator.clone(),
                        context,
                    )) as Box<dyn BoxedPane>,
                )),
                _ => None,
            })
        })
    }

    pub fn get_mut<'pane_ref, 'pane_type_ref: 'pane_ref>(
        &'pane_ref mut self,
        pane: &'pane_type_ref PaneType,
        context: &AppContext,
    ) -> Result<Panes<'pane_ref, 'panes>> {
        match pane {
            PaneType::Queue => Ok(Panes::Queue(&mut self.queue)),
            #[cfg(debug_assertions)]
            PaneType::Logs => Ok(Panes::Logs(&mut self.logs)),
            PaneType::Directories => Ok(Panes::Directories(&mut self.directories)),
            PaneType::Artists => Ok(Panes::Artists(&mut self.artists)),
            PaneType::AlbumArtists => Ok(Panes::AlbumArtists(&mut self.album_artists)),
            PaneType::Albums => Ok(Panes::Albums(&mut self.albums)),
            PaneType::Playlists => Ok(Panes::Playlists(&mut self.playlists)),
            PaneType::Search => Ok(Panes::Search(&mut self.search)),
            PaneType::AlbumArt => Ok(Panes::AlbumArt(&mut self.album_art)),
            PaneType::Lyrics => Ok(Panes::Lyrics(&mut self.lyrics)),
            PaneType::ProgressBar => Ok(Panes::ProgressBar(&mut self.progress_bar)),
            PaneType::Header => Ok(Panes::Header(&mut self.header)),
            PaneType::Tabs => Ok(Panes::Tabs(&mut self.tabs)),
            PaneType::TabContent => Ok(Panes::TabContent),
            #[cfg(debug_assertions)]
            PaneType::FrameCount => Ok(Panes::FrameCount(&mut self.frame_count)),
            PaneType::Property { content, align, scroll_speed } => {
                Ok(Panes::Property(PropertyPane::<'pane_type_ref>::new(
                    content,
                    *align,
                    (*scroll_speed).into(),
                    context,
                )))
            }
            p @ PaneType::Browser { .. } => Ok(Panes::Others(
                self.others
                    .get_mut(pane)
                    .with_context(|| format!("expected pane to be defined {p:?}"))?,
            )),
        }
    }
}

macro_rules! pane_call {
    ($screen:ident, $fn:ident($($param:expr),+)) => {
        match &mut $screen {
            Panes::Queue(s) => s.$fn($($param),+),
            #[cfg(debug_assertions)]
            Panes::Logs(s) => s.$fn($($param),+),
            Panes::Directories(s) => s.$fn($($param),+),
            Panes::Artists(s) => s.$fn($($param),+),
            Panes::AlbumArtists(s) => s.$fn($($param),+),
            Panes::Albums(s) => s.$fn($($param),+),
            Panes::Playlists(s) => s.$fn($($param),+),
            Panes::Search(s) => s.$fn($($param),+),
            Panes::AlbumArt(s) => s.$fn($($param),+),
            Panes::Lyrics(s) => s.$fn($($param),+),
            Panes::ProgressBar(s) => s.$fn($($param),+),
            Panes::Header(s) => s.$fn($($param),+),
            Panes::Tabs(s) => s.$fn($($param),+),
            Panes::TabContent => Ok(()),
            #[cfg(debug_assertions)]
            Panes::FrameCount(s) => s.$fn($($param),+),
            Panes::Property(s) => s.$fn($($param),+),
            Panes::Others(s) => s.$fn($($param),+),
        }
    }
}
pub(crate) use pane_call;

#[allow(unused_variables)]
pub(crate) trait Pane {
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
    fn on_event(
        &mut self,
        event: &mut UiEvent,
        is_visible: bool,
        context: &AppContext,
    ) -> Result<()> {
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

    use itertools::Itertools;
    use ratatui::{
        style::{Color, Style},
        text::{Line, Span},
    };

    use crate::{
        config::theme::properties::SongProperty,
        mpd::commands::{Song, lsinfo::LsInfoEntry},
        shared::mpd_query::PreviewGroup,
    };

    impl Song {
        pub(crate) fn to_preview(&self) -> Vec<PreviewGroup> {
            let key_style = Style::default().fg(Color::Yellow);
            let separator = Span::from(": ");
            let start_of_line_spacer = Span::from(" ");

            let mut info_group = PreviewGroup::new(Some(" --- [Info]"));

            let file = Line::from(vec![
                start_of_line_spacer.clone(),
                Span::styled("File", key_style),
                separator.clone(),
                Span::from(self.file.clone()),
            ]);
            info_group.push(file.into());

            if let Some(file_name) = self.file_name() {
                info_group.push(
                    Line::from(vec![
                        start_of_line_spacer.clone(),
                        Span::styled("Filename", key_style),
                        separator.clone(),
                        Span::from(file_name.into_owned()),
                    ])
                    .into(),
                );
            }

            if let Some(title) = self.metadata.get("title") {
                title.for_each(|item| {
                    info_group.push(
                        Line::from(vec![
                            start_of_line_spacer.clone(),
                            Span::styled("Title", key_style),
                            separator.clone(),
                            Span::from(item.to_owned()),
                        ])
                        .into(),
                    );
                });
            }
            if let Some(artist) = self.metadata.get("artist") {
                artist.for_each(|item| {
                    info_group.push(
                        Line::from(vec![
                            start_of_line_spacer.clone(),
                            Span::styled("Artist", key_style),
                            separator.clone(),
                            Span::from(item.to_owned()),
                        ])
                        .into(),
                    );
                });
            }

            if let Some(album) = self.metadata.get("album") {
                album.for_each(|item| {
                    info_group.push(
                        Line::from(vec![
                            start_of_line_spacer.clone(),
                            Span::styled("Album", key_style),
                            separator.clone(),
                            Span::from(item.to_owned()),
                        ])
                        .into(),
                    );
                });
            }

            if let Some(duration) = &self.duration {
                info_group.push(
                    Line::from(vec![
                        start_of_line_spacer.clone(),
                        Span::styled("Duration", key_style),
                        separator.clone(),
                        Span::from(duration.as_secs().to_string()),
                    ])
                    .into(),
                );
            }

            let mut tags_group = PreviewGroup::new(Some(" --- [Tags]"));
            for (k, v) in self
                .metadata
                .iter()
                .filter(|(key, _)| {
                    !["title", "album", "artist", "duration"].contains(&(*key).as_str())
                })
                .sorted_by_key(|(key, _)| *key)
            {
                v.for_each(|item| {
                    tags_group.push(
                        Line::from(vec![
                            start_of_line_spacer.clone(),
                            Span::styled(k.clone(), key_style),
                            separator.clone(),
                            Span::from(item.to_owned()),
                        ])
                        .into(),
                    );
                });
            }

            vec![info_group, tags_group]
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum DirOrSong {
        Dir { name: String, full_path: String },
        Song(Song),
    }

    impl DirOrSong {
        pub fn name_only(name: String) -> Self {
            DirOrSong::Dir { name, full_path: String::new() }
        }

        pub fn dir_name_or_file_name(&self) -> Cow<str> {
            match self {
                DirOrSong::Dir { name, full_path: _ } => Cow::Borrowed(name),
                DirOrSong::Song(song) => Cow::Borrowed(&song.file),
            }
        }
    }

    impl Ord for DirOrSong {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            match (self, other) {
                (DirOrSong::Dir { name: a, .. }, DirOrSong::Dir { name: b, .. }) => a.cmp(b),
                (DirOrSong::Song(_), DirOrSong::Dir { .. }) => Ordering::Greater,
                (DirOrSong::Dir { .. }, DirOrSong::Song(_)) => Ordering::Less,
                (DirOrSong::Song(a), DirOrSong::Song(b)) => a.cmp_by_prop(b, &SongProperty::Title),
            }
        }
    }

    impl PartialOrd for DirOrSong {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub(crate) struct SongCustomSort<'a>(&'a Song, &'a [SongProperty]);

    impl Song {
        pub(crate) fn with_custom_sort<'a>(
            &'a self,
            sort_props: &'a [SongProperty],
        ) -> SongCustomSort<'a> {
            SongCustomSort(self, sort_props)
        }
    }

    impl Ord for SongCustomSort<'_> {
        fn cmp(&self, other: &Self) -> Ordering {
            for prop in self.1 {
                let ord = self.0.cmp_by_prop(other.0, prop);
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            Ordering::Equal
        }
    }

    impl PartialOrd for SongCustomSort<'_> {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub(crate) struct DirOrSongCustomSort<'a>(&'a DirOrSong, &'a [SongProperty]);

    impl DirOrSong {
        pub(crate) fn with_custom_sort<'a>(
            &'a self,
            sort_props: &'a [SongProperty],
        ) -> DirOrSongCustomSort<'a> {
            DirOrSongCustomSort(self, sort_props)
        }
    }

    impl Ord for DirOrSongCustomSort<'_> {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            match (self.0, other.0) {
                (DirOrSong::Dir { name: a, .. }, DirOrSong::Dir { name: b, .. }) => a.cmp(b),
                (DirOrSong::Song(_), DirOrSong::Dir { .. }) => Ordering::Greater,
                (DirOrSong::Dir { .. }, DirOrSong::Song(_)) => Ordering::Less,
                (DirOrSong::Song(a), DirOrSong::Song(b)) => {
                    a.with_custom_sort(self.1).cmp(&b.with_custom_sort(self.1))
                }
            }
        }
    }

    impl PartialOrd for DirOrSongCustomSort<'_> {
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

        use super::DirOrSong;
        use crate::mpd::commands::Song;

        #[test]
        fn dir_before_song() {
            let mut input = vec![
                DirOrSong::Song(Song::default()),
                DirOrSong::Dir { name: "a".to_owned(), full_path: String::new() },
                DirOrSong::Song(Song::default()),
                DirOrSong::Dir { name: "z".to_owned(), full_path: String::new() },
                DirOrSong::Song(Song::default()),
            ];

            input.sort();

            assert_eq!(
                input,
                vec![
                    DirOrSong::Dir { name: "a".to_owned(), full_path: String::new() },
                    DirOrSong::Dir { name: "z".to_owned(), full_path: String::new() },
                    DirOrSong::Song(Song::default()),
                    DirOrSong::Song(Song::default()),
                    DirOrSong::Song(Song::default()),
                ]
            );
        }
    }
}

impl Song {
    pub fn title_str(&self, separator: &str) -> Cow<'_, str> {
        self.metadata.get("title").map_or(Cow::Borrowed("Untitled"), |v| v.join(separator))
    }

    pub fn artist_str(&self, separator: &str) -> Cow<'_, str> {
        self.metadata.get("artist").map_or(Cow::Borrowed("Unknown"), |v| v.join(separator))
    }

    pub fn file_name(&self) -> Option<Cow<str>> {
        std::path::Path::new(&self.file).file_stem().map(|file_name| file_name.to_string_lossy())
    }

    pub fn file_ext(&self) -> Option<Cow<str>> {
        std::path::Path::new(&self.file).extension().map(|ext| ext.to_string_lossy())
    }

    fn format<'song>(
        &'song self,
        property: &SongProperty,
        tag_separator: &str,
    ) -> Option<Cow<'song, str>> {
        match property {
            SongProperty::Filename => self.file_name(),
            SongProperty::FileExtension => self.file_ext(),
            SongProperty::File => Some(Cow::Borrowed(self.file.as_str())),
            SongProperty::Title => self.metadata.get("title").map(|v| v.join(tag_separator)),
            SongProperty::Artist => self.metadata.get("artist").map(|v| v.join(tag_separator)),
            SongProperty::Album => self.metadata.get("album").map(|v| v.join(tag_separator)),
            SongProperty::Duration => self.duration.map(|d| Cow::Owned(d.to_string())),
            SongProperty::Disc => self.metadata.get("disc").map(|v| Cow::Borrowed(v.last())),
            SongProperty::Other(name) => self.metadata.get(name).map(|v| v.join(tag_separator)),
            SongProperty::Track => self.metadata.get("track").map(|v| {
                Cow::Owned(
                    v.last()
                        .parse::<u32>()
                        .map_or_else(|_| v.last().to_owned(), |v| format!("{v:0>2}")),
                )
            }),
        }
    }

    fn cmp_by_prop(&self, other: &Self, property: &SongProperty) -> Ordering {
        match property {
            SongProperty::Filename => match (self.file_name(), other.file_name()) {
                (Some(a), Some(b)) => UniCase::new(a).cmp(&UniCase::new(b)),
                (_, Some(_)) => Ordering::Greater,
                (Some(_), _) => Ordering::Less,
                (None, None) => Ordering::Equal,
            },
            SongProperty::FileExtension => match (self.file_ext(), other.file_ext()) {
                (Some(a), Some(b)) => UniCase::new(a).cmp(&UniCase::new(b)),
                (_, Some(_)) => Ordering::Greater,
                (Some(_), _) => Ordering::Less,
                (None, None) => Ordering::Equal,
            },
            SongProperty::File => UniCase::new(&self.file).cmp(&UniCase::new(&other.file)),
            SongProperty::Title => {
                match (
                    self.metadata.get("title").map(|v| v.join("")),
                    other.metadata.get("title").map(|v| v.join("")),
                ) {
                    (Some(a), Some(b)) => UniCase::new(a).cmp(&UniCase::new(b)),
                    (_, Some(_)) => Ordering::Greater,
                    (Some(_), _) => Ordering::Less,
                    (None, None) => Ordering::Equal,
                }
            }
            SongProperty::Artist => {
                match (
                    self.metadata.get("artist").map(|v| v.join("")),
                    other.metadata.get("artist").map(|v| v.join("")),
                ) {
                    (Some(a), Some(b)) => UniCase::new(a).cmp(&UniCase::new(b)),
                    (_, Some(_)) => Ordering::Greater,
                    (Some(_), _) => Ordering::Less,
                    (None, None) => Ordering::Equal,
                }
            }
            SongProperty::Album => {
                match (
                    self.metadata.get("album").map(|v| v.join("")),
                    other.metadata.get("album").map(|v| v.join("")),
                ) {
                    (Some(a), Some(b)) => UniCase::new(a).cmp(&UniCase::new(b)),
                    (_, Some(_)) => Ordering::Greater,
                    (Some(_), _) => Ordering::Less,
                    (None, None) => Ordering::Equal,
                }
            }
            SongProperty::Track => {
                let self_track = self.metadata.get("track");
                let self_track = self_track.map(|v| v.join(""));
                let other_track = other.metadata.get("track");
                let other_track = other_track.map(|v| v.join(""));
                match (self_track, other_track) {
                    (Some(a), Some(b)) => match (a.parse::<i32>(), b.parse::<i32>()) {
                        (Ok(a), Ok(b)) => a.cmp(&b),
                        _ => UniCase::new(a).cmp(&UniCase::new(b)),
                    },
                    (_, Some(_)) => Ordering::Greater,
                    (Some(_), _) => Ordering::Less,
                    (None, None) => Ordering::Equal,
                }
            }
            SongProperty::Disc => {
                let self_disc = self.metadata.get("disc");
                let self_disc = self_disc.map(|v| v.join(""));
                let other_disc = other.metadata.get("disc");
                let other_disc = other_disc.map(|v| v.join(""));
                match (self_disc, other_disc) {
                    (Some(a), Some(b)) => match (a.parse::<i32>(), b.parse::<i32>()) {
                        (Ok(a), Ok(b)) => a.cmp(&b),
                        _ => UniCase::new(a).cmp(&UniCase::new(b)),
                    },
                    (_, Some(_)) => Ordering::Greater,
                    (Some(_), _) => Ordering::Less,
                    (None, None) => Ordering::Equal,
                }
            }
            SongProperty::Other(prop_name) => {
                match (
                    self.metadata.get(prop_name).map(|v| v.join("")),
                    other.metadata.get(prop_name).map(|v| v.join("")),
                ) {
                    (Some(a), Some(b)) => UniCase::new(a).cmp(&UniCase::new(b)),
                    (_, Some(_)) => Ordering::Greater,
                    (Some(_), _) => Ordering::Less,
                    (None, None) => Ordering::Equal,
                }
            }
            SongProperty::Duration => match (self.duration, other.duration) {
                (Some(a), Some(b)) => a.as_millis().cmp(&b.as_millis()),
                (_, Some(_)) => Ordering::Greater,
                (Some(_), _) => Ordering::Less,
                (None, None) => Ordering::Equal,
            },
        }
    }

    pub fn matches<'a>(
        &self,
        formats: impl IntoIterator<Item = &'a Property<SongProperty>>,
        filter: &str,
    ) -> bool {
        for format in formats {
            let match_found = match &format.kind {
                PropertyKindOrText::Text(value) => {
                    Some(value.to_lowercase().contains(&filter.to_lowercase()))
                }
                PropertyKindOrText::Sticker(key) => self
                    .stickers
                    .as_ref()
                    .and_then(|stickers| {
                        stickers
                            .get(key)
                            .map(|value| value.to_lowercase().contains(&filter.to_lowercase()))
                    })
                    .or_else(|| {
                        format
                            .default
                            .as_ref()
                            .map(|f| self.matches(std::iter::once(f.as_ref()), filter))
                    }),
                PropertyKindOrText::Property(property) => self.format(property, "").map_or_else(
                    || {
                        format
                            .default
                            .as_ref()
                            .map(|f| self.matches(std::iter::once(f.as_ref()), filter))
                    },
                    |p| Some(p.to_lowercase().contains(&filter.to_lowercase())),
                ),
                PropertyKindOrText::Group(_) => format
                    .as_string(Some(self), "")
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
        format: &Property<SongProperty>,
        max_len: usize,
        symbols: &SymbolsConfig,
        tag_separator: &str,
    ) -> Option<Line<'song>> {
        format
            .default
            .as_ref()
            .and_then(|f| self.as_line_ellipsized(f.as_ref(), max_len, symbols, tag_separator))
    }

    pub fn as_line_ellipsized<'song>(
        &'song self,
        format: &Property<SongProperty>,
        max_len: usize,
        symbols: &SymbolsConfig,
        tag_separator: &str,
    ) -> Option<Line<'song>> {
        let style = format.style.unwrap_or_default();
        match &format.kind {
            PropertyKindOrText::Text(value) => {
                Some(Line::styled((*value).ellipsize(max_len, symbols).to_string(), style))
            }
            PropertyKindOrText::Sticker(key) => self
                .stickers
                .as_ref()
                .and_then(|stickers| stickers.get(key))
                .map(|sticker| Line::styled(sticker.ellipsize(max_len, symbols), style))
                .or_else(|| {
                    format.default.as_ref().and_then(|format| {
                        self.as_line_ellipsized(format.as_ref(), max_len, symbols, tag_separator)
                    })
                }),
            PropertyKindOrText::Property(property) => {
                self.format(property, tag_separator).map_or_else(
                    || self.default_as_line_ellipsized(format, max_len, symbols, tag_separator),
                    |v| Some(Line::styled(v.ellipsize(max_len, symbols).into_owned(), style)),
                )
            }
            PropertyKindOrText::Group(group) => {
                let mut buf = Line::default();
                for grformat in group {
                    if let Some(res) =
                        self.as_line_ellipsized(grformat, max_len, symbols, tag_separator)
                    {
                        for span in res.spans {
                            buf.push_span(span);
                        }
                    } else {
                        return format.default.as_ref().and_then(|format| {
                            self.as_line_ellipsized(format, max_len, symbols, tag_separator)
                        });
                    }
                }
                return Some(buf);
            }
        }
    }
}

impl Property<SongProperty> {
    fn default(&self, song: Option<&Song>, tag_separator: &str) -> Option<String> {
        self.default.as_ref().and_then(|p| p.as_string(song, tag_separator))
    }

    pub fn as_string(&self, song: Option<&Song>, tag_separator: &str) -> Option<String> {
        match &self.kind {
            PropertyKindOrText::Text(value) => Some((*value).to_string()),
            PropertyKindOrText::Sticker(key) => {
                if let Some(sticker) =
                    song.map(|s| s.stickers.as_ref().and_then(|stickers| stickers.get(key)))
                {
                    sticker.cloned()
                } else {
                    self.default(song, tag_separator)
                }
            }
            PropertyKindOrText::Property(property) => {
                if let Some(song) = song {
                    song.format(property, tag_separator).map_or_else(
                        || self.default(Some(song), tag_separator),
                        |v| Some(v.into_owned()),
                    )
                } else {
                    self.default(song, tag_separator)
                }
            }
            PropertyKindOrText::Group(group) => {
                let mut buf = String::new();
                for format in group {
                    if let Some(res) = format.as_string(song, tag_separator) {
                        buf.push_str(&res);
                    } else {
                        return self
                            .default
                            .as_ref()
                            .and_then(|d| d.as_string(song, tag_separator));
                    }
                }
                return Some(buf);
            }
        }
    }
}

impl Property<PropertyKind> {
    fn default_as_span<'song: 's, 's>(
        &'s self,
        song: Option<&'song Song>,
        status: &'song Status,
        tag_separator: &str,
    ) -> Option<Either<Span<'s>, Vec<Span<'s>>>> {
        self.default.as_ref().and_then(|p| p.as_span(song, status, tag_separator))
    }

    pub fn as_span<'song: 's, 's>(
        &'s self,
        song: Option<&'song Song>,
        status: &'song Status,
        tag_separator: &str,
    ) -> Option<Either<Span<'s>, Vec<Span<'s>>>> {
        let style = self.style.unwrap_or_default();
        match &self.kind {
            PropertyKindOrText::Text(value) => Some(Either::Left(Span::styled(value, style))),
            PropertyKindOrText::Sticker(key) => {
                if let Some(sticker) =
                    song.and_then(|s| s.stickers.as_ref().and_then(|stickers| stickers.get(key)))
                {
                    Some(Either::Left(Span::styled(sticker, style)))
                } else {
                    self.default_as_span(song, status, tag_separator)
                }
            }
            PropertyKindOrText::Property(PropertyKind::Song(property)) => {
                if let Some(song) = song {
                    song.format(property, tag_separator).map_or_else(
                        || self.default_as_span(Some(song), status, tag_separator),
                        |s| Some(Either::Left(Span::styled(s, style))),
                    )
                } else {
                    self.default_as_span(song, status, tag_separator)
                }
            }
            PropertyKindOrText::Property(PropertyKind::Status(s)) => match s {
                StatusProperty::State {
                    playing_label,
                    paused_label,
                    stopped_label,
                    playing_style,
                    paused_style,
                    stopped_style,
                } => Some(Either::Left(Span::styled(
                    match status.state {
                        State::Play => playing_label,
                        State::Stop => stopped_label,
                        State::Pause => paused_label,
                    },
                    match status.state {
                        State::Play => playing_style,
                        State::Stop => stopped_style,
                        State::Pause => paused_style,
                    }
                    .unwrap_or(style),
                ))),
                StatusProperty::Duration => {
                    Some(Either::Left(Span::styled(status.duration.to_string(), style)))
                }
                StatusProperty::Elapsed => {
                    Some(Either::Left(Span::styled(status.elapsed.to_string(), style)))
                }
                StatusProperty::Volume => {
                    Some(Either::Left(Span::styled(status.volume.value().to_string(), style)))
                }
                StatusProperty::Repeat { on_label, off_label, on_style, off_style } => {
                    Some(Either::Left(Span::styled(
                        if status.repeat { on_label } else { off_label },
                        if status.repeat { on_style } else { off_style }.unwrap_or(style),
                    )))
                }
                StatusProperty::Random { on_label, off_label, on_style, off_style } => {
                    Some(Either::Left(Span::styled(
                        if status.random { on_label } else { off_label },
                        if status.random { on_style } else { off_style }.unwrap_or(style),
                    )))
                }
                StatusProperty::Consume {
                    on_label,
                    off_label,
                    oneshot_label,
                    on_style,
                    off_style,
                    oneshot_style,
                } => Some(Either::Left(Span::styled(
                    match status.consume {
                        OnOffOneshot::On => on_label,
                        OnOffOneshot::Off => off_label,
                        OnOffOneshot::Oneshot => oneshot_label,
                    },
                    match status.consume {
                        OnOffOneshot::On => on_style,
                        OnOffOneshot::Off => off_style,
                        OnOffOneshot::Oneshot => oneshot_style,
                    }
                    .unwrap_or(style),
                ))),
                StatusProperty::Single {
                    on_label,
                    off_label,
                    oneshot_label,
                    on_style,
                    off_style,
                    oneshot_style,
                } => Some(Either::Left(Span::styled(
                    match status.single {
                        OnOffOneshot::On => on_label,
                        OnOffOneshot::Off => off_label,
                        OnOffOneshot::Oneshot => oneshot_label,
                    },
                    match status.single {
                        OnOffOneshot::On => on_style,
                        OnOffOneshot::Off => off_style,
                        OnOffOneshot::Oneshot => oneshot_style,
                    }
                    .unwrap_or(style),
                ))),
                StatusProperty::Bitrate => status.bitrate.as_ref().map_or_else(
                    || self.default_as_span(song, status, tag_separator),
                    |v| Some(Either::Left(Span::styled(v.to_string(), style))),
                ),
                StatusProperty::Crossfade => status.xfade.as_ref().map_or_else(
                    || self.default_as_span(song, status, tag_separator),
                    |v| Some(Either::Left(Span::styled(v.to_string(), style))),
                ),
            },
            PropertyKindOrText::Property(PropertyKind::Widget(w)) => match w {
                WidgetProperty::Volume => {
                    Some(Either::Left(Span::styled(Volume::get_str(*status.volume.value()), style)))
                }
                WidgetProperty::States { active_style, separator_style } => {
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
                WidgetProperty::ScanStatus => {
                    // FIXME: state not persisting, this is wrong and only here
                    // to make compiler happy
                    let mut state = ScanStatusState::new(status.updating_db);
                    Some(Either::Left(Span::styled(
                        ScanStatus::default().get_str(&mut state),
                        style,
                    )))
                }
            },
            PropertyKindOrText::Group(group) => {
                let mut buf = Vec::new();
                for format in group {
                    match format.as_span(song, status, tag_separator) {
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
        area: Rect,
        pane_callback: &mut impl FnMut(&ConfigPane, Rect, Block, Rect) -> Result<()>,
    ) -> Result<()> {
        self.for_each_pane_custom_data(
            area,
            (),
            &mut |pane, pane_area, block, block_area, ()| {
                pane_callback(pane, pane_area, block, block_area)?;
                Ok(())
            },
            &mut |_, _, ()| Ok(()),
        )
    }

    pub fn for_each_pane_custom_data<T>(
        &self,
        area: Rect,
        mut custom_data: T,
        pane_callback: &mut impl FnMut(&ConfigPane, Rect, Block, Rect, &mut T) -> Result<()>,
        split_callback: &mut impl FnMut(Block, Rect, &mut T) -> Result<()>,
    ) -> Result<()> {
        let mut stack = vec![(self, area)];

        while let Some((configured_panes, area)) = stack.pop() {
            match configured_panes {
                SizedPaneOrSplit::Pane(pane) => {
                    let block = Block::default().borders(pane.borders);
                    let pane_area = block.inner(area);

                    pane_callback(pane, pane_area, block, area, &mut custom_data)?;
                }
                SizedPaneOrSplit::Split { direction, panes, borders } => {
                    let constraints = panes.iter().map(|pane| Into::<Constraint>::into(pane.size));
                    let block = Block::default().borders(*borders);
                    let pane_areas = block.inner(area);
                    let areas = Layout::new(*direction, constraints).split(pane_areas);

                    split_callback(block, area, &mut custom_data)?;

                    stack.extend(
                        areas.iter().enumerate().map(|(idx, area)| (&panes[idx].pane, *area)),
                    );
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
#[allow(clippy::unwrap_used)]
mod format_tests {
    use crate::{
        config::theme::properties::{Property, PropertyKindOrText, SongProperty},
        mpd::commands::Song,
    };

    mod correct_values {
        use std::{collections::HashMap, time::Duration};

        use either::Either;
        use ratatui::{
            style::{Style, Stylize},
            text::Span,
        };
        use test_case::test_case;

        use super::*;
        use crate::{
            config::theme::{
                StyleFile,
                properties::{PropertyKind, StatusProperty, StatusPropertyFile},
            },
            mpd::commands::{State, Status, Volume, status::OnOffOneshot},
        };

        #[test_case(SongProperty::Title, "title")]
        #[test_case(SongProperty::Artist, "artist")]
        #[test_case(SongProperty::Album, "album")]
        #[test_case(SongProperty::Track, "123")]
        #[test_case(SongProperty::Duration, "2:03")]
        #[test_case(SongProperty::Other("track".to_string()), "123")]
        fn song_property_resolves_correctly(prop: SongProperty, expected: &str) {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Property(prop),
                style: None,
                default: None,
            };

            let song = Song {
                id: 123,
                file: "file".to_owned(),
                duration: Some(Duration::from_secs(123)),
                metadata: HashMap::from([
                    ("title".to_string(), "title".into()),
                    ("album".to_string(), "album".into()),
                    ("track".to_string(), "123".into()),
                    ("artist".to_string(), "artist".into()),
                ]),
                stickers: None,
            };

            let result = format.as_string(Some(&song), "");

            assert_eq!(result, Some(expected.to_string()));
        }

        #[test_case(StatusProperty::Volume, "100")]
        #[test_case(StatusProperty::Elapsed, "2:03")]
        #[test_case(StatusProperty::Duration, "2:03")]
        #[test_case(StatusProperty::Crossfade, "3")]
        #[test_case(StatusProperty::Bitrate, "123")]
        fn status_property_resolves_correctly(prop: StatusProperty, expected: &str) {
            let format = Property::<PropertyKind> {
                kind: PropertyKindOrText::Property(PropertyKind::Status(prop)),
                style: None,
                default: None,
            };

            let song = Song {
                id: 123,
                file: "file".to_owned(),
                duration: Some(Duration::from_secs(123)),
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".into()),
                    ("album".to_string(), "album".into()),
                    ("title".to_string(), "title".into()),
                    ("track".to_string(), "123".into()),
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

            let result = format.as_span(Some(&song), &status, "");

            assert_eq!(
                result,
                Some(either::Either::<Span<'_>, Vec<Span<'_>>>::Left(Span::raw(expected)))
            );
        }

        #[test_case("otherplay", "otherstopped", "otherpaused", State::Play, "otherplay")]
        #[test_case("otherplay", "otherstopped", "otherpaused", State::Pause, "otherpaused")]
        #[test_case("otherplay", "otherstopped", "otherpaused", State::Stop, "otherstopped")]
        fn playback_state_label_is_correct(
            playing_label: &'static str,
            stopped_label: &'static str,
            paused_label: &'static str,
            state: State,
            expected_label: &str,
        ) {
            let format = Property::<PropertyKind> {
                kind: PropertyKindOrText::Property(PropertyKind::Status(StatusProperty::State {
                    playing_label: playing_label.to_string(),
                    paused_label: paused_label.to_string(),
                    stopped_label: stopped_label.to_string(),
                    playing_style: None,
                    paused_style: None,
                    stopped_style: None,
                })),
                style: None,
                default: None,
            };

            let song = Song { id: 1, file: "file".to_owned(), ..Default::default() };
            let status = Status { state, ..Default::default() };

            let result = format.as_span(Some(&song), &status, "");

            assert_eq!(
                result,
                Some(either::Either::<Span<'_>, Vec<Span<'_>>>::Left(Span::raw(expected_label)))
            );
        }

        #[test_case(StatusPropertyFile::ConsumeV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), oneshot_label: "1111".to_string(), on_style: None, off_style: None, oneshot_style: None }, &Status { consume: OnOffOneshot::On, ..Default::default() }, "ye")]
        #[test_case(StatusPropertyFile::ConsumeV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), oneshot_label: "1111".to_string(), on_style: None, off_style: None, oneshot_style: None }, &Status { consume: OnOffOneshot::Off, ..Default::default() }, "naw")]
        #[test_case(StatusPropertyFile::ConsumeV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), oneshot_label: "1111".to_string(), on_style: None, off_style: None, oneshot_style: None }, &Status { consume: OnOffOneshot::Oneshot, ..Default::default() }, "1111")]
        #[test_case(StatusPropertyFile::SingleV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), oneshot_label: "1111".to_string(), on_style: None, off_style: None, oneshot_style: None }, &Status { single: OnOffOneshot::On, ..Default::default() }, "ye")]
        #[test_case(StatusPropertyFile::SingleV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), oneshot_label: "1111".to_string(), on_style: None, off_style: None, oneshot_style: None }, &Status { single: OnOffOneshot::Off, ..Default::default() }, "naw")]
        #[test_case(StatusPropertyFile::SingleV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), oneshot_label: "1111".to_string(), on_style: None, off_style: None, oneshot_style: None }, &Status { single: OnOffOneshot::Oneshot, ..Default::default() }, "1111")]
        #[test_case(StatusPropertyFile::RandomV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), on_style: None, off_style: None }, &Status { random: true, ..Default::default() }, "ye")]
        #[test_case(StatusPropertyFile::RandomV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), on_style: None, off_style: None }, &Status { random: false, ..Default::default() }, "naw")]
        #[test_case(StatusPropertyFile::RepeatV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), on_style: None, off_style: None }, &Status { repeat: true, ..Default::default() }, "ye")]
        #[test_case(StatusPropertyFile::RepeatV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), on_style: None, off_style: None }, &Status { repeat: false, ..Default::default() }, "naw")]
        #[test_case(StatusPropertyFile::Consume, &Status { consume: OnOffOneshot::On, ..Default::default() }, "On")]
        #[test_case(StatusPropertyFile::Consume, &Status { consume: OnOffOneshot::Off, ..Default::default() }, "Off")]
        #[test_case(StatusPropertyFile::Consume, &Status { consume: OnOffOneshot::Oneshot, ..Default::default() }, "OS")]
        #[test_case(StatusPropertyFile::Repeat, &Status { repeat: true, ..Default::default() }, "On")]
        #[test_case(StatusPropertyFile::Repeat, &Status { repeat: false, ..Default::default() }, "Off")]
        #[test_case(StatusPropertyFile::Random, &Status { random: true, ..Default::default() }, "On")]
        #[test_case(StatusPropertyFile::Random, &Status { random: false, ..Default::default() }, "Off")]
        #[test_case(StatusPropertyFile::Single, &Status { single: OnOffOneshot::On, ..Default::default() }, "On")]
        #[test_case(StatusPropertyFile::Single, &Status { single: OnOffOneshot::Off, ..Default::default() }, "Off")]
        #[test_case(StatusPropertyFile::Single, &Status { single: OnOffOneshot::Oneshot, ..Default::default() }, "OS")]
        fn on_off_states_label_is_correct(
            prop: StatusPropertyFile,
            status: &Status,
            expected_label: &str,
        ) {
            let format = Property::<PropertyKind> {
                kind: PropertyKindOrText::Property(PropertyKind::Status(prop.try_into().unwrap())),
                style: None,
                default: None,
            };

            let song = Song { id: 1, file: "file".to_owned(), ..Default::default() };

            let result = format.as_span(Some(&song), status, "");

            assert_eq!(result, Some(Either::Left(Span::raw(expected_label))));
        }

        #[test_case(StatusPropertyFile::ConsumeV2 { on_style: Some(StyleFile::builder().fg("red".to_string()).build()), off_style: Some(StyleFile::builder().fg("green".to_string()).build()), oneshot_style: Some(StyleFile::builder().fg("blue".to_string()).build()), on_label: String::new(), off_label: String::new(), oneshot_label: String::new() }, &Status { consume: OnOffOneshot::On, ..Default::default() }, Some(Style::default().red()))]
        #[test_case(StatusPropertyFile::SingleV2  { on_style: Some(StyleFile::builder().fg("red".to_string()).build()), off_style: Some(StyleFile::builder().fg("green".to_string()).build()), oneshot_style: Some(StyleFile::builder().fg("blue".to_string()).build()),  on_label: String::new(), off_label: String::new(), oneshot_label: String::new() }, &Status { single: OnOffOneshot::On, ..Default::default() }, Some(Style::default().red()))]
        #[test_case(StatusPropertyFile::RandomV2  { on_style: Some(StyleFile::builder().fg("red".to_string()).build()), off_style: Some(StyleFile::builder().fg("green".to_string()).build()), on_label: String::new(), off_label: String::new() }, &Status { random: true, ..Default::default() }, Some(Style::default().red()))]
        #[test_case(StatusPropertyFile::RepeatV2  { on_style: Some(StyleFile::builder().fg("red".to_string()).build()), off_style: Some(StyleFile::builder().fg("green".to_string()).build()), on_label: String::new(), off_label: String::new() }, &Status { repeat: true, ..Default::default() }, Some(Style::default().red()))]
        #[test_case(StatusPropertyFile::ConsumeV2 { on_style: None, off_style: None, oneshot_style: None, on_label: String::new(), off_label: String::new(), oneshot_label: String::new() }, &Status { consume: OnOffOneshot::On, ..Default::default() }, None)]
        #[test_case(StatusPropertyFile::SingleV2  { on_style: None, off_style: None, oneshot_style: None, on_label: String::new(), off_label: String::new(), oneshot_label: String::new() }, &Status { single: OnOffOneshot::On, ..Default::default() }, None)]
        #[test_case(StatusPropertyFile::RandomV2  { on_style: None, off_style: None, on_label: String::new(), off_label: String::new() }, &Status { random: true, ..Default::default() }, None)]
        #[test_case(StatusPropertyFile::RepeatV2  { on_style: None, off_style: None, on_label: String::new(), off_label: String::new() }, &Status { repeat: true, ..Default::default() }, None)]
        fn on_off_oneshot_styles_are_correct(
            prop: StatusPropertyFile,
            status: &Status,
            expected_style: Option<Style>,
        ) {
            let format = Property::<PropertyKind> {
                kind: PropertyKindOrText::Property(PropertyKind::Status(prop.try_into().unwrap())),
                style: None,
                default: None,
            };

            let song = Song { id: 1, file: "file".to_owned(), ..Default::default() };

            let result = format.as_span(Some(&song), status, "");

            dbg!(&result);
            assert_eq!(
                result,
                Some(Either::Left(Span::styled(String::new(), expected_style.unwrap_or_default())))
            );
        }
    }

    mod property {
        use std::collections::HashMap;

        use super::*;

        #[test]
        fn works() {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Property(SongProperty::Title),
                style: None,
                default: None,
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".into()),
                    ("title".to_string(), "title".into()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song), "");

            assert_eq!(result, Some("title".to_owned()));
        }

        #[test]
        fn falls_back() {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Property(SongProperty::Track),
                style: None,
                default: Some(
                    Property {
                        kind: PropertyKindOrText::Text("fallback".into()),
                        style: None,
                        default: None,
                    }
                    .into(),
                ),
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".into()),
                    ("title".to_string(), "title".into()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song), "");

            assert_eq!(result, Some("fallback".to_owned()));
        }

        #[test]
        fn falls_back_to_none() {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Property(SongProperty::Track),
                style: None,
                default: None,
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".into()),
                    ("title".to_string(), "title".into()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song), "");

            assert_eq!(result, None);
        }
    }

    mod text {
        use std::collections::HashMap;

        use super::*;

        #[test]
        fn works() {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Text("test".into()),
                style: None,
                default: None,
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".into()),
                    ("title".to_string(), "title".into()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song), "");

            assert_eq!(result, Some("test".to_owned()));
        }

        #[test]
        fn fallback_is_ignored() {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Text("test".into()),
                style: None,
                default: Some(
                    Property {
                        kind: PropertyKindOrText::Text("fallback".into()),
                        style: None,
                        default: None,
                    }
                    .into(),
                ),
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".into()),
                    ("title".to_string(), "title".into()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song), "");

            assert_eq!(result, Some("test".to_owned()));
        }
    }

    mod group {
        use std::collections::HashMap;

        use super::*;

        #[test]
        fn group_no_fallback() {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Group(vec![
                    Property {
                        kind: PropertyKindOrText::Property(SongProperty::Track),
                        style: None,
                        default: None,
                    },
                    Property {
                        kind: PropertyKindOrText::Text(" ".into()),
                        style: None,
                        default: None,
                    },
                ]),
                style: None,
                default: None,
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".into()),
                    ("title".to_string(), "title".into()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song), "");

            assert_eq!(result, None);
        }

        #[test]
        fn group_fallback() {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Group(vec![
                    Property {
                        kind: PropertyKindOrText::Property(SongProperty::Track),
                        style: None,
                        default: None,
                    },
                    Property {
                        kind: PropertyKindOrText::Text(" ".into()),
                        style: None,
                        default: None,
                    },
                ]),
                style: None,
                default: Some(
                    Property {
                        kind: PropertyKindOrText::Text("fallback".into()),
                        style: None,
                        default: None,
                    }
                    .into(),
                ),
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".into()),
                    ("title".to_string(), "title".into()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song), "");

            assert_eq!(result, Some("fallback".to_owned()));
        }

        #[test]
        fn group_resolved() {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Group(vec![
                    Property {
                        kind: PropertyKindOrText::Property(SongProperty::Title),
                        style: None,
                        default: None,
                    },
                    Property {
                        kind: PropertyKindOrText::Text("text".into()),
                        style: None,
                        default: None,
                    },
                ]),
                style: None,
                default: Some(
                    Property {
                        kind: PropertyKindOrText::Text("fallback".into()),
                        style: None,
                        default: None,
                    }
                    .into(),
                ),
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".into()),
                    ("title".to_string(), "title".into()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song), "");

            assert_eq!(result, Some("titletext".to_owned()));
        }

        #[test]
        fn group_fallback_in_group() {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Group(vec![
                    Property {
                        kind: PropertyKindOrText::Property(SongProperty::Track),
                        style: None,
                        default: Some(
                            Property {
                                kind: PropertyKindOrText::Text("fallback".into()),
                                style: None,
                                default: None,
                            }
                            .into(),
                        ),
                    },
                    Property {
                        kind: PropertyKindOrText::Text("text".into()),
                        style: None,
                        default: None,
                    },
                ]),
                style: None,
                default: None,
            };

            let song = Song {
                metadata: HashMap::from([
                    ("artist".to_string(), "artist".into()),
                    ("title".to_string(), "title".into()),
                ]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song), "");

            assert_eq!(result, Some("fallbacktext".to_owned()));
        }

        #[test]
        fn group_nesting() {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Group(vec![
                    Property {
                        kind: PropertyKindOrText::Group(vec![
                            Property {
                                kind: PropertyKindOrText::Property(SongProperty::Track),
                                style: None,
                                default: None,
                            },
                            Property {
                                kind: PropertyKindOrText::Text("inner".into()),
                                style: None,
                                default: None,
                            },
                        ]),
                        style: None,
                        default: Some(
                            Property {
                                kind: PropertyKindOrText::Text("innerfallback".into()),
                                style: None,
                                default: None,
                            }
                            .into(),
                        ),
                    },
                    Property {
                        kind: PropertyKindOrText::Text("outer".into()),
                        style: None,
                        default: None,
                    },
                ]),
                style: None,
                default: None,
            };

            let song = Song {
                metadata: HashMap::from([("title".to_string(), "title".into())]),
                ..Default::default()
            };

            let result = format.as_string(Some(&song), "");

            assert_eq!(result, Some("innerfallbackouter".to_owned()));
        }
    }
}
