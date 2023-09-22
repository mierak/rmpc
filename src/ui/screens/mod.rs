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

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
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

pub mod dirstack {
    use ratatui::widgets::{ListItem, ListState, ScrollbarState, TableState};

    use crate::mpd::commands::lsinfo::FileOrDir;

    #[derive(Debug)]
    pub struct DirStack<T: std::fmt::Debug + MatchesSearch> {
        pub current: (Vec<T>, MyState<ListState>),
        others: Vec<(Vec<T>, MyState<ListState>)>,
        pub preview: Option<Vec<ListItem<'static>>>,
        pub filter: Option<String>,
        pub filter_ignore_case: bool,
    }

    impl<T: std::fmt::Debug + MatchesSearch> DirStack<T> {
        pub fn new(root: Vec<T>) -> Self {
            let mut result = Self {
                others: Vec::new(),
                current: (Vec::new(), MyState::default()),
                filter: None,
                filter_ignore_case: true,
                preview: None,
            };
            let mut root_state = MyState::default();

            result.push(Vec::new());

            if !root.is_empty() {
                root_state.select(Some(0));
                // root.sort();
            };

            result.current = (root, root_state);
            result
        }

        /// Returns the element at the top of the stack
        pub fn current(&mut self) -> (&Vec<T>, &mut MyState<ListState>) {
            (&self.current.0, &mut self.current.1)
        }

        /// Returns the element at the second element from the top of the stack
        pub fn previous(&mut self) -> (&Vec<T>, &mut MyState<ListState>) {
            let last = self
                .others
                .last_mut()
                .expect("Previous items to always containt at least one item. This should have been handled in pop()");
            (&last.0, &mut last.1)
        }

        /// Returns the element at the second element from the top of the stack
        pub fn preview(&self) -> Option<&Vec<ListItem<'static>>> {
            self.preview.as_ref()
        }

        pub fn replace_current(&mut self, new_current: Vec<T>) {
            if new_current.is_empty() {
                self.current.1.select(None);
            } else if self.current.1.get_selected().is_some_and(|v| v > new_current.len() - 1) {
                self.current.1.select(Some(new_current.len() - 1));
            }
            self.current.0 = new_current;
        }

        pub fn push(&mut self, head: Vec<T>) {
            let mut new_state = MyState::default();
            if !head.is_empty() {
                new_state.select(Some(0));
            };
            let current_head = std::mem::replace(&mut self.current, (head, new_state));
            self.others.push(current_head);
            self.filter = None;
        }

        pub fn pop(&mut self) -> Option<(Vec<T>, MyState<ListState>)> {
            if self.others.len() > 1 {
                self.filter = None;
                let top = self.others.pop().expect("There should always be at least two elements");
                Some(std::mem::replace(&mut self.current, top))
            } else {
                None
            }
        }

        pub fn get_selected(&self) -> Option<&T> {
            if let Some(sel) = self.current.1.get_selected() {
                self.current.0.get(sel)
            } else {
                None
            }
        }

        pub fn get_previous_selected(&self) -> Option<&T> {
            let previous = self
                .others
                .last()
                .expect("Previous items to always containt at least one item. This should have been handled in pop()");
            if let Some(sel) = previous.1.get_selected() {
                previous.0.get(sel)
            } else {
                None
            }
        }

        pub fn next(&mut self) {
            self.current.1.next();
        }

        pub fn prev(&mut self) {
            self.current.1.prev();
        }

        pub fn next_half_viewport(&mut self) {
            self.current.1.next_half_viewport();
        }

        pub fn prev_half_viewport(&mut self) {
            self.current.1.prev_half_viewport();
        }

        pub fn last(&mut self) {
            self.current.1.last();
        }

        pub fn first(&mut self) {
            self.current.1.first();
        }

        pub fn jump_forward(&mut self) {
            if let Some(filter) = self.filter.as_ref() {
                if let Some(selected) = self.current.1.get_selected() {
                    for i in selected + 1..self.current.0.len() {
                        let s = &self.current.0[i];
                        if s.matches(filter, self.filter_ignore_case) {
                            self.current.1.select(Some(i));
                            break;
                        }
                    }
                }
            }
        }

        pub fn jump_back(&mut self) {
            if let Some(filter) = self.filter.as_ref() {
                if let Some(selected) = self.current.1.get_selected() {
                    for i in (0..selected).rev() {
                        let s = &self.current.0[i];
                        if s.matches(filter, self.filter_ignore_case) {
                            self.current.1.select(Some(i));
                            break;
                        }
                    }
                }
            }
        }
    }

    #[derive(Debug, Default)]
    pub struct MyState<T: ScrollingState> {
        pub scrollbar_state: ScrollbarState,
        pub inner: T,
        pub content_len: Option<u16>,
        pub viewport_len: Option<u16>,
    }

    impl<T: ScrollingState> MyState<T> {
        pub fn viewport_len(&mut self, viewport_len: Option<u16>) -> &Self {
            self.viewport_len = viewport_len;
            self.scrollbar_state = self.scrollbar_state.viewport_content_length(viewport_len.unwrap_or(0));
            self
        }

        pub fn content_len(&mut self, content_len: Option<u16>) -> &Self {
            self.content_len = content_len;
            self.scrollbar_state = self.scrollbar_state.content_length(content_len.unwrap_or(0));
            self
        }

        pub fn first(&mut self) {
            if self.content_len.is_some() {
                self.select(Some(0));
            } else {
                self.select(None);
            }
        }

        pub fn last(&mut self) {
            if let Some(item_count) = self.content_len {
                self.select(Some(item_count.saturating_sub(1) as usize));
            } else {
                self.select(None);
            }
        }

        pub fn next(&mut self) {
            if let Some(item_count) = self.content_len {
                let i = match self.get_selected() {
                    Some(i) => {
                        if i >= item_count.saturating_sub(1) as usize {
                            Some(0)
                        } else {
                            Some(i + 1)
                        }
                    }
                    None => None,
                };
                self.select(i);
            } else {
                self.select(None);
            }
        }

        pub fn prev(&mut self) {
            if let Some(item_count) = self.content_len {
                let i = match self.get_selected() {
                    Some(i) => {
                        if i == 0 {
                            Some(item_count.saturating_sub(1) as usize)
                        } else {
                            Some(i - 1)
                        }
                    }
                    None => None,
                };
                self.select(i);
            } else {
                self.select(None);
            }
        }

        pub fn next_half_viewport(&mut self) {
            if let Some(item_count) = self.content_len {
                if let Some(viewport) = self.viewport_len {
                    self.select(self.get_selected().map(|i| {
                        i.saturating_add(viewport as usize / 2)
                            .min(item_count.saturating_sub(1) as usize)
                    }));
                } else {
                    self.select(None);
                }
            } else {
                self.select(None);
            }
        }

        pub fn prev_half_viewport(&mut self) {
            if self.content_len.is_some() {
                if let Some(viewport) = self.viewport_len {
                    self.select(
                        self.get_selected()
                            .map(|i| i.saturating_sub(viewport as usize / 2).max(0)),
                    );
                } else {
                    self.select(None);
                }
            } else {
                self.select(None);
            }
        }

        #[allow(clippy::cast_possible_truncation)]
        pub fn select(&mut self, idx: Option<usize>) {
            self.inner.select_scrolling(idx);
            self.scrollbar_state = self.scrollbar_state.position(idx.unwrap_or(0) as u16);
        }

        pub fn get_selected(&self) -> Option<usize> {
            self.inner.get_selected_scrolling()
        }
    }

    pub trait ScrollingState {
        fn select_scrolling(&mut self, idx: Option<usize>);
        fn get_selected_scrolling(&self) -> Option<usize>;
    }

    impl ScrollingState for TableState {
        fn select_scrolling(&mut self, idx: Option<usize>) {
            self.select(idx);
        }

        fn get_selected_scrolling(&self) -> Option<usize> {
            self.selected()
        }
    }

    impl ScrollingState for ListState {
        fn select_scrolling(&mut self, idx: Option<usize>) {
            self.select(idx);
        }

        fn get_selected_scrolling(&self) -> Option<usize> {
            self.selected()
        }
    }

    pub trait MatchesSearch {
        fn matches(&self, filter: &str, ignorecase: bool) -> bool;
    }

    impl MatchesSearch for String {
        fn matches(&self, filter: &str, ignorecase: bool) -> bool {
            if ignorecase {
                self.to_lowercase().contains(&filter.to_lowercase())
            } else {
                self.contains(filter)
            }
        }
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

    #[cfg(test)]
    mod tests {
        use super::DirStack;

        #[test]
        fn leaves_at_least_one_element_in_others() {
            let mut val: DirStack<String> = DirStack::new(Vec::new());
            val.push(Vec::new());
            assert!(val.pop().is_some());
            assert!(val.pop().is_none());

            val.previous();
        }
    }
}

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
    };

    use super::dirstack::MatchesSearch;

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

    impl DirOrSong {
        pub fn to_current_value(&self) -> &str {
            match self {
                DirOrSong::Dir(d) => d,
                DirOrSong::Song(s) => s,
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
            match (self, other) {
                (DirOrSong::Dir(a), DirOrSong::Dir(b)) => Some(a.cmp(b)),
                (_, DirOrSong::Dir(_)) => Some(Ordering::Greater),
                (DirOrSong::Dir(_), _) => Some(Ordering::Less),
                (DirOrSong::Song(a), DirOrSong::Song(b)) => Some(a.cmp(b)),
            }
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
            match (self, other) {
                (DirOrSongInfo::Dir(a), DirOrSongInfo::Dir(b)) => Some(a.cmp(b)),
                (_, DirOrSongInfo::Dir(_)) => Some(Ordering::Greater),
                (DirOrSongInfo::Dir(_), _) => Some(Ordering::Less),
                (DirOrSongInfo::Song(Song { title: t1, .. }), DirOrSongInfo::Song(Song { title: t2, .. })) => {
                    Some(t1.cmp(t2))
                }
            }
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
    use ratatui::widgets::ListItem;

    use crate::config::SymbolsConfig;

    use super::browser::{DirOrSong, DirOrSongInfo};

    pub struct BrowserItemInfo<'a, I> {
        iter: I,
        symbols: &'a SymbolsConfig,
    }

    impl<I> Iterator for BrowserItemInfo<'_, I>
    where
        I: Iterator<Item = DirOrSongInfo>,
    {
        type Item = ListItem<'static>;

        fn next(&mut self) -> Option<Self::Item> {
            match self.iter.next() {
                Some(v) => match v {
                    DirOrSongInfo::Dir(v) => Some(ListItem::new(format!("{} {}", self.symbols.dir, v.as_str()))),
                    DirOrSongInfo::Song(s) => Some(ListItem::new(format!(
                        "{} {}",
                        self.symbols.song,
                        s.title.as_ref().map_or("Untitled", |v| v.as_str())
                    ))),
                },
                None => None,
            }
        }
    }

    pub trait DirOrSongInfoListItems<T> {
        fn listitems(self, symbols: &SymbolsConfig) -> BrowserItemInfo<T>;
    }
    impl<T: Iterator<Item = DirOrSongInfo>> DirOrSongInfoListItems<T> for T {
        fn listitems(self, symbols: &SymbolsConfig) -> BrowserItemInfo<T> {
            BrowserItemInfo { iter: self, symbols }
        }
    }

    pub struct BrowserItem<'a, I> {
        iter: I,
        symbols: &'a SymbolsConfig,
    }

    impl<I> Iterator for BrowserItem<'_, I>
    where
        I: Iterator<Item = DirOrSong>,
    {
        type Item = ListItem<'static>;

        fn next(&mut self) -> Option<Self::Item> {
            match self.iter.next() {
                Some(v) => match v {
                    DirOrSong::Dir(v) => Some(ListItem::new(format!(
                        "{} {}",
                        self.symbols.dir,
                        if v.is_empty() { "Untitled" } else { v.as_str() }
                    ))),
                    DirOrSong::Song(s) => Some(ListItem::new(format!("{} {}", self.symbols.song, s))),
                },
                None => None,
            }
        }
    }
    pub trait DirOrSongListItems<T> {
        fn listitems(self, symbols: &SymbolsConfig) -> BrowserItem<T>;
    }

    impl<T: Iterator<Item = DirOrSong>> DirOrSongListItems<T> for T {
        fn listitems(self, symbols: &SymbolsConfig) -> BrowserItem<T> {
            BrowserItem { iter: self, symbols }
        }
    }
}
