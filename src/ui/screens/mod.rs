use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::KeyEvent;
use ratatui::{
    prelude::{Backend, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::ListItem,
    Frame,
};
use strum::{Display, EnumIter, EnumVariantNames};

use crate::{
    mpd::{client::Client, commands::Song},
    state::State,
};

use super::{KeyHandleResult, SharedUiState};

pub mod albums;
pub mod artists;
pub mod directories;
pub mod logs;
pub mod queue;

#[derive(Debug, Display, EnumVariantNames, Default, Clone, Copy, EnumIter, PartialEq)]
pub enum Screens {
    #[default]
    Queue,
    Logs,
    Directories,
    Artists,
    Albums,
}

#[async_trait]
pub trait Screen {
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

    async fn handle_action(
        &mut self,
        event: KeyEvent,
        _client: &mut Client<'_>,
        _app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<KeyHandleResult>;
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
            Screens::Queue => Screens::Logs,
            Screens::Logs => Screens::Directories,
            Screens::Directories => Screens::Artists,
            Screens::Artists => Screens::Albums,
            Screens::Albums => Screens::Queue,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Screens::Queue => Screens::Albums,
            Screens::Albums => Screens::Artists,
            Screens::Artists => Screens::Directories,
            Screens::Directories => Screens::Logs,
            Screens::Logs => Screens::Queue,
        }
    }
}

pub mod dirstack {
    use ratatui::widgets::{ListState, ScrollbarState, TableState};

    use crate::mpd::commands::lsinfo::FileOrDir;

    #[derive(Debug)]
    pub struct DirStack<T: std::fmt::Debug + MatchesSearch> {
        current: (Vec<T>, MyState<ListState>),
        others: Vec<(Vec<T>, MyState<ListState>)>,
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
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.select(Some(i));
            } else {
                self.select(None);
            }
        }

        pub fn prev(&mut self) {
            if let Some(item_count) = self.content_len {
                let i = match self.get_selected() {
                    Some(i) => {
                        if i == 0 {
                            item_count.saturating_sub(1) as usize
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.select(Some(i));
            } else {
                self.select(None);
            }
        }

        pub fn next_half_viewport(&mut self) {
            if let Some(item_count) = self.content_len {
                if let Some(viewport) = self.viewport_len {
                    let i = match self.get_selected() {
                        Some(i) => i
                            .saturating_add(viewport as usize / 2)
                            .min(item_count.saturating_sub(1) as usize),
                        None => 0,
                    };
                    self.select(Some(i));
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
                    let i = match self.get_selected() {
                        Some(i) => i.saturating_sub(viewport as usize / 2).max(0),
                        None => 0,
                    };
                    self.select(Some(i));
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

pub trait ToListItems {
    fn to_listitems(&self, state: &State) -> Vec<ListItem<'static>>;
}

impl ToListItems for Song {
    fn to_listitems(&self, _app: &State) -> Vec<ListItem<'static>> {
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
            Span::from(": "),
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
        let r = vec![title, artist, album, duration];
        let r = [
            r,
            self.others
                .iter()
                .map(|(k, v)| {
                    Line::from(vec![
                        start_of_line_spacer.clone(),
                        Span::styled(k.clone(), key_style),
                        separator.clone(),
                        Span::from(v.clone()),
                    ])
                })
                .collect(),
        ]
        .concat();

        r.into_iter().map(ListItem::new).collect()
    }
}

pub trait SongOrTagExt {
    fn to_listitems(&self, is_song: bool, state: &State) -> Vec<ListItem<'static>>;
}

impl SongOrTagExt for Vec<String> {
    fn to_listitems(&self, is_song: bool, state: &State) -> Vec<ListItem<'static>> {
        self.iter()
            .map(|val| {
                ListItem::new(format!(
                    "{} {}",
                    if is_song {
                        state.config.symbols.song
                    } else {
                        state.config.symbols.dir
                    },
                    if val.is_empty() { "Unknown" } else { val }
                ))
            })
            .collect::<Vec<ListItem>>()
    }
}
