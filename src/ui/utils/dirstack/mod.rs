use ratatui::{
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
    config::Config,
    mpd::commands::Song,
    ui::screens::{browser::DirOrSong, StringExt},
};

pub trait DirStackItem {
    type Item;
    fn as_path(&self) -> &str;
    fn matches(&self, filter: &str, ignore_case: bool) -> bool;
    fn to_list_item(&self, config: &Config, is_marked: bool, filter: Option<&str>) -> Self::Item;
}

impl<'a> DirStackItem for &'a str {
    type Item = ListItem<'a>;

    fn as_path(&self) -> &str {
        self
    }

    fn matches(&self, filter: &str, ignorecase: bool) -> bool {
        if ignorecase {
            self.to_lowercase().contains(&filter.to_lowercase())
        } else {
            self.contains(filter)
        }
    }

    fn to_list_item(&self, config: &Config, _is_marked: bool, filter: Option<&str>) -> Self::Item {
        if filter.is_some_and(|filter| self.matches(filter, true)) {
            ListItem::new(self.to_owned()).style(config.theme.highlighted_item_style)
        } else {
            ListItem::new(self.to_owned())
        }
    }
}

impl DirStackItem for String {
    type Item = ListItem<'static>;

    fn as_path(&self) -> &str {
        self
    }

    fn matches(&self, filter: &str, ignorecase: bool) -> bool {
        if ignorecase {
            self.to_lowercase().contains(&filter.to_lowercase())
        } else {
            self.contains(filter)
        }
    }

    fn to_list_item(&self, config: &Config, is_marked: bool, filter: Option<&str>) -> Self::Item {
        let symbols = &config.theme.symbols;
        let marker_span = if is_marked {
            Span::styled(symbols.marker, config.theme.highlighted_item_style)
        } else {
            Span::from(" ".repeat(symbols.marker.chars().count()))
        };

        if filter.is_some_and(|filter| self.matches(filter, true)) {
            ListItem::new(Line::from(vec![marker_span, Span::from(self.clone())]))
                .style(config.theme.highlighted_item_style)
        } else {
            ListItem::new(Line::from(vec![marker_span, Span::from(self.clone())]))
        }
    }
}

impl DirStackItem for DirOrSong {
    type Item = ListItem<'static>;

    fn as_path(&self) -> &str {
        match self {
            DirOrSong::Dir(d) => d,
            DirOrSong::Song(s) => s,
        }
    }

    fn matches(&self, filter: &str, ignorecase: bool) -> bool {
        if ignorecase {
            match self {
                DirOrSong::Dir(v) => if v.is_empty() { "Untitled" } else { v.as_str() }
                    .to_lowercase()
                    .contains(&filter.to_lowercase()),
                DirOrSong::Song(s) => s.to_lowercase().contains(&filter.to_lowercase()),
            }
        } else {
            match self {
                DirOrSong::Dir(v) => if v.is_empty() { "Untitled" } else { v.as_str() }.contains(filter),
                DirOrSong::Song(s) => s.contains(filter),
            }
        }
    }

    fn to_list_item(&self, config: &Config, is_marked: bool, filter: Option<&str>) -> Self::Item {
        let symbols = &config.theme.symbols;
        let marker_span = if is_marked {
            Span::styled(symbols.marker, config.theme.highlighted_item_style)
        } else {
            Span::from(" ".repeat(symbols.marker.chars().count()))
        };

        let value = match self {
            DirOrSong::Dir(v) => format!("{} {}", symbols.dir, if v.is_empty() { "Untitled" } else { v.as_str() }),
            DirOrSong::Song(s) => format!("{} {}", symbols.song, s.file_name()),
        };
        if filter.is_some_and(|filter| self.matches(filter, true)) {
            ListItem::new(Line::from(vec![marker_span, Span::from(value)])).style(config.theme.highlighted_item_style)
        } else {
            ListItem::new(Line::from(vec![marker_span, Span::from(value)]))
        }
    }
}

impl DirStackItem for Song {
    type Item = ListItem<'static>;

    fn as_path(&self) -> &str {
        &self.file
    }

    fn matches(&self, filter: &str, ignore_case: bool) -> bool {
        if ignore_case {
            format!("{} - {}", self.title_str(), self.artist_str())
                .to_lowercase()
                .contains(&filter.to_lowercase())
        } else {
            format!("{} - {}", self.title_str(), self.artist_str()).contains(filter)
        }
    }

    fn to_list_item(&self, config: &Config, is_marked: bool, filter: Option<&str>) -> Self::Item {
        let symbols = &config.theme.symbols;
        let marker_span = if is_marked {
            Span::styled(symbols.marker, config.theme.highlighted_item_style)
        } else {
            Span::from(" ".repeat(symbols.marker.chars().count()))
        };

        let title = self.title_str().to_owned();
        let artist = self.artist_str().to_owned();
        let separator_span = Span::from(" - ");
        let icon_span = Span::from(format!(" {}", symbols.song));
        let mut result = ListItem::new(Line::from(vec![
            icon_span,
            marker_span,
            Span::from(artist),
            separator_span,
            Span::from(title),
        ]));
        if filter.is_some_and(|filter| DirStackItem::matches(self, filter, true)) {
            result = result.style(config.theme.highlighted_item_style);
        }

        result
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
