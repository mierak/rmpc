use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::KeyEvent;
use ratatui::{
    prelude::{Backend, Rect},
    Frame,
};
use strum::{Display, EnumIter, EnumVariantNames};

use crate::{
    mpd::{client::Client, commands::Song},
    state::State,
};

use super::{KeyHandleResultInternal, SharedUiState};

pub mod albums;
pub mod artists;
pub mod directories;
pub mod logs;
pub mod playlists;
pub mod queue;

#[derive(Debug, Display, EnumVariantNames, Default, Clone, Copy, EnumIter, PartialEq)]
pub enum Screens {
    #[default]
    Queue,
    #[cfg(debug_assertions)]
    Logs,
    Directories,
    Artists,
    Albums,
    Playlists,
}

#[async_trait]
pub(super) trait Screen {
    type Actions;
    fn render<B: Backend>(
        &mut self,
        frame: &mut Frame<B>,
        area: Rect,
        app: &mut crate::state::State,
        shared_state: &mut SharedUiState,
    ) -> Result<()>;

    /// For any cleanup operations, ran when the screen hides
    async fn on_hide(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared_state: &mut SharedUiState,
    ) -> Result<()> {
        Ok(())
    }

    /// For work that needs to be done BEFORE the first render
    async fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        Ok(())
    }

    /// Used to keep the current state but refresh data
    async fn refresh(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        Ok(())
    }

    async fn handle_action(
        &mut self,
        event: KeyEvent,
        _client: &mut Client<'_>,
        _app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<KeyHandleResultInternal>;
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum CommonAction {
    Down,
    Up,
    DownHalf,
    UpHalf,
    Right,
    Left,
    Top,
    Bottom,
    EnterSearch,
    NextResult,
    PreviousResult,
    Select,
}

impl Screens {
    pub fn next(self) -> Self {
        match self {
            #[cfg(debug_assertions)]
            Screens::Queue => Screens::Logs,
            #[cfg(not(debug_assertions))]
            Screens::Queue => Screens::Directories,
            #[cfg(debug_assertions)]
            Screens::Logs => Screens::Directories,
            Screens::Directories => Screens::Artists,
            Screens::Artists => Screens::Albums,
            Screens::Albums => Screens::Playlists,
            Screens::Playlists => Screens::Queue,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Screens::Queue => Screens::Playlists,
            Screens::Playlists => Screens::Albums,
            Screens::Albums => Screens::Artists,
            Screens::Artists => Screens::Directories,
            #[cfg(not(debug_assertions))]
            Screens::Directories => Screens::Queue,
            #[cfg(debug_assertions)]
            Screens::Directories => Screens::Logs,
            #[cfg(debug_assertions)]
            Screens::Logs => Screens::Queue,
        }
    }
}

pub mod dirstack {}

pub(crate) mod browser {
    use std::cmp::Ordering;

    use ratatui::{
        style::{Color, Style},
        text::{Line, Span},
        widgets::ListItem,
    };

    use crate::{
        config::SymbolsConfig,
        mpd::commands::{lsinfo::FileOrDir, Song},
        ui::utils::dirstack::{AsPath, MatchesSearch},
    };

    pub trait ToListItems {
        fn to_listitems(&self, symbols: &SymbolsConfig) -> Vec<ListItem<'static>>;
    }

    impl ToListItems for Song {
        fn to_listitems(&self, _symbols: &SymbolsConfig) -> Vec<ListItem<'static>> {
            let key_style = Style::default().fg(Color::Yellow);
            let separator = Span::from(": ");
            let start_of_line_spacer = Span::from(" ");

            let title = Line::from(vec![
                start_of_line_spacer.clone(),
                Span::styled("Title", key_style),
                separator.clone(),
                Span::from(self.title.as_ref().map_or("Untitled", |v| v.as_str()).to_owned()),
            ]);
            let artist = Line::from(vec![
                start_of_line_spacer.clone(),
                Span::styled("Artist", key_style),
                separator.clone(),
                Span::from(self.artist.as_ref().map_or("Unknown", |v| v.as_str()).to_owned()),
            ]);
            let album = Line::from(vec![
                start_of_line_spacer.clone(),
                Span::styled("Album", key_style),
                separator.clone(),
                Span::from(self.album.as_ref().map_or("Unknown", |v| v.as_str()).to_owned()),
            ]);
            let duration = Line::from(vec![
                start_of_line_spacer.clone(),
                Span::styled("Duration", key_style),
                separator.clone(),
                Span::from(
                    self.duration
                        .as_ref()
                        .map_or("-".to_owned(), |v| v.as_secs().to_string()),
                ),
            ]);
            let mut r = vec![title, artist, album, duration];
            for (k, v) in &self.others {
                r.push(Line::from(vec![
                    start_of_line_spacer.clone(),
                    Span::styled(k.clone(), key_style),
                    separator.clone(),
                    Span::from(v.clone()),
                ]));
            }

            r.into_iter().map(ListItem::new).collect()
        }
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum DirOrSong {
        Dir(String),
        Song(String),
    }

    impl MatchesSearch for FileOrDir {
        fn matches(&self, filter: &str, ignorecase: bool) -> bool {
            if ignorecase {
                match self {
                    FileOrDir::Dir(dir) => dir.path.to_lowercase().contains(&filter.to_lowercase()),
                    FileOrDir::File(song) => song
                        .title
                        .as_ref()
                        .is_some_and(|s| s.to_lowercase().contains(&filter.to_lowercase())),
                }
            } else {
                match self {
                    FileOrDir::Dir(dir) => dir.path.contains(filter),
                    FileOrDir::File(song) => song.title.as_ref().is_some_and(|s| s.contains(filter)),
                }
            }
        }
    }

    impl MatchesSearch for DirOrSong {
        fn matches(&self, filter: &str, ignorecase: bool) -> bool {
            if ignorecase {
                match self {
                    DirOrSong::Dir(v) => v.to_lowercase().contains(&filter.to_lowercase()),
                    DirOrSong::Song(s) => s.to_lowercase().contains(&filter.to_lowercase()),
                }
            } else {
                match self {
                    DirOrSong::Dir(v) => v.contains(filter),
                    DirOrSong::Song(s) => s.contains(filter),
                }
            }
        }
    }

    impl AsPath for DirOrSong {
        fn as_path(&self) -> Option<&str> {
            match self {
                DirOrSong::Dir(d) => Some(d),
                DirOrSong::Song(s) => Some(s),
            }
        }
    }

    impl std::cmp::Ord for DirOrSong {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            match (self, other) {
                (DirOrSong::Dir(a), DirOrSong::Dir(b)) => a.cmp(b),
                (_, DirOrSong::Dir(_)) => Ordering::Greater,
                (DirOrSong::Dir(_), _) => Ordering::Less,
                (DirOrSong::Song(a), DirOrSong::Song(b)) => a.cmp(b),
            }
        }
    }
    impl std::cmp::PartialOrd for DirOrSong {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum DirOrSongInfo {
        Dir(String),
        Song(Song),
    }

    impl MatchesSearch for DirOrSongInfo {
        fn matches(&self, filter: &str, ignorecase: bool) -> bool {
            if ignorecase {
                match self {
                    DirOrSongInfo::Dir(v) => v.to_lowercase().contains(&filter.to_lowercase()),
                    DirOrSongInfo::Song(s) => s
                        .title
                        .as_ref()
                        .map_or("Untitled", |v| v.as_str())
                        .to_lowercase()
                        .contains(&filter.to_lowercase()),
                }
            } else {
                match self {
                    DirOrSongInfo::Dir(v) => v.contains(filter),
                    DirOrSongInfo::Song(s) => s.title.as_ref().map_or("Untitled", |v| v.as_str()).contains(filter),
                }
            }
        }
    }

    impl AsPath for DirOrSongInfo {
        fn as_path(&self) -> Option<&str> {
            match self {
                DirOrSongInfo::Dir(d) => Some(d),
                DirOrSongInfo::Song(s) => s.title.as_deref(),
            }
        }
    }

    impl From<FileOrDir> for DirOrSongInfo {
        fn from(value: FileOrDir) -> Self {
            match value {
                FileOrDir::Dir(dir) => DirOrSongInfo::Dir(dir.path),
                FileOrDir::File(song) => DirOrSongInfo::Song(song),
            }
        }
    }

    impl std::cmp::Ord for DirOrSongInfo {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            match (self, other) {
                (DirOrSongInfo::Dir(a), DirOrSongInfo::Dir(b)) => a.cmp(b),
                (_, DirOrSongInfo::Dir(_)) => Ordering::Greater,
                (DirOrSongInfo::Dir(_), _) => Ordering::Less,
                (DirOrSongInfo::Song(Song { title: t1, .. }), DirOrSongInfo::Song(Song { title: t2, .. })) => {
                    t1.cmp(t2)
                }
            }
        }
    }
    impl std::cmp::PartialOrd for DirOrSongInfo {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
}

pub trait SongExt {
    fn title_str(&self) -> &str;
    fn artist_str(&self) -> &str;
}

impl SongExt for Song {
    fn title_str(&self) -> &str {
        self.title.as_ref().map_or("Untitled", |v| v.as_str())
    }

    fn artist_str(&self) -> &str {
        self.artist.as_ref().map_or("Untitled", |v| v.as_str())
    }
}

pub mod iter {
    use std::{collections::BTreeSet, ops::AddAssign};

    use ratatui::{
        style::{Color, Style},
        text::{Line, Span},
        widgets::ListItem,
    };

    use crate::config::SymbolsConfig;

    use super::browser::{DirOrSong, DirOrSongInfo};

    pub struct BrowserItemInfo<'a, I> {
        iter: I,
        symbols: &'a SymbolsConfig,
        marked: &'a BTreeSet<usize>,
        count: usize,
    }

    impl<I> Iterator for BrowserItemInfo<'_, I>
    where
        I: Iterator<Item = DirOrSongInfo>,
    {
        type Item = ListItem<'static>;

        fn next(&mut self) -> Option<Self::Item> {
            let result = match self.iter.next() {
                Some(v) => {
                    let marker_span = if self.marked.contains(&self.count) {
                        Span::styled(self.symbols.marker, Style::default().fg(Color::Blue))
                    } else {
                        Span::from(" ".repeat(self.symbols.marker.chars().count()))
                    };

                    let value = match v {
                        DirOrSongInfo::Dir(v) => format!("{} {}", self.symbols.dir, v.as_str()),
                        DirOrSongInfo::Song(s) => format!(
                            "{} {}",
                            self.symbols.song,
                            s.title.as_ref().map_or("Untitled", |v| v.as_str())
                        ),
                    };
                    Some(ListItem::new(Line::from(vec![marker_span, Span::from(value)])))
                }
                None => None,
            };
            self.count.add_assign(1);
            result
        }
    }

    pub trait DirOrSongInfoListItems<T> {
        fn listitems<'a>(self, symbols: &'a SymbolsConfig, marked: &'a BTreeSet<usize>) -> BrowserItemInfo<'a, T>;
    }
    impl<T: Iterator<Item = DirOrSongInfo>> DirOrSongInfoListItems<T> for T {
        fn listitems<'a>(self, symbols: &'a SymbolsConfig, marked: &'a BTreeSet<usize>) -> BrowserItemInfo<'a, T> {
            BrowserItemInfo {
                iter: self,
                count: 0,
                symbols,
                marked,
            }
        }
    }

    pub struct BrowserItem<'a, I> {
        iter: I,
        count: usize,
        symbols: &'a SymbolsConfig,
        marked: &'a BTreeSet<usize>,
    }

    impl<I> Iterator for BrowserItem<'_, I>
    where
        I: Iterator<Item = DirOrSong>,
    {
        type Item = ListItem<'static>;

        fn next(&mut self) -> Option<Self::Item> {
            let result = match self.iter.next() {
                Some(v) => {
                    let marker_span = if self.marked.contains(&self.count) {
                        Span::styled(self.symbols.marker, Style::default().fg(Color::Blue))
                    } else {
                        Span::from(" ".repeat(self.symbols.marker.chars().count()))
                    };
                    let value = match v {
                        DirOrSong::Dir(v) => format!(
                            "{} {}",
                            self.symbols.dir,
                            if v.is_empty() { "Untitled" } else { v.as_str() }
                        ),
                        DirOrSong::Song(s) => format!("{} {}", self.symbols.song, s),
                    };
                    Some(ListItem::new(Line::from(vec![marker_span, Span::from(value)])))
                }
                None => None,
            };
            self.count.add_assign(1);
            result
        }
    }
    pub trait DirOrSongListItems<T> {
        fn listitems<'a>(self, symbols: &'a SymbolsConfig, marked: &'a BTreeSet<usize>) -> BrowserItem<'a, T>;
    }

    impl<T: Iterator<Item = DirOrSong>> DirOrSongListItems<T> for T {
        fn listitems<'a>(self, symbols: &'a SymbolsConfig, marked: &'a BTreeSet<usize>) -> BrowserItem<'a, T> {
            BrowserItem {
                iter: self,
                count: 0,
                symbols,
                marked,
            }
        }
    }
}
