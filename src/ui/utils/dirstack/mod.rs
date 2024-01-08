use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{ListItem, ListState, TableState},
};

mod dir;
mod stack;
mod state;
pub use dir::Dir;
pub use stack::DirStack;
pub use state::DirState;

use crate::{
    config::SymbolsConfig,
    ui::screens::{browser::DirOrSong, StringExt},
};

pub trait DirStackItem {
    fn as_path(&self) -> Option<&str>;
    fn matches(&self, filter: &str, ignorecase: bool) -> bool;
    fn to_list_item(&self, symbols: &SymbolsConfig, is_marked: bool) -> ListItem<'static>;
}

impl DirStackItem for String {
    fn as_path(&self) -> Option<&str> {
        Some(self)
    }

    fn matches(&self, filter: &str, ignorecase: bool) -> bool {
        if ignorecase {
            self.to_lowercase().contains(&filter.to_lowercase())
        } else {
            self.contains(filter)
        }
    }

    fn to_list_item(&self, _symbols: &SymbolsConfig, _is_marked: bool) -> ListItem<'static> {
        ListItem::new(self.clone())
    }
}

impl DirStackItem for DirOrSong {
    fn as_path(&self) -> Option<&str> {
        match self {
            DirOrSong::Dir(d) => Some(d),
            DirOrSong::Song(s) => Some(s),
        }
    }

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

    fn to_list_item(&self, symbols: &SymbolsConfig, is_marked: bool) -> ListItem<'static> {
        let marker_span = if is_marked {
            Span::styled(symbols.marker, Style::default().fg(Color::Blue))
        } else {
            Span::from(" ".repeat(symbols.marker.chars().count()))
        };

        let value = match self {
            DirOrSong::Dir(v) => format!("{} {}", symbols.dir, if v.is_empty() { "Untitled" } else { v.as_str() }),
            DirOrSong::Song(s) => format!("{} {}", symbols.song, s.file_name()),
        };
        ListItem::new(Line::from(vec![marker_span, Span::from(value)]))
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
