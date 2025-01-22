use itertools::Itertools;
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListItem, ListState, TableState};

mod dir;
mod stack;
mod state;
pub use dir::Dir;
pub use stack::DirStack;
pub use state::DirState;

use crate::config::Config;
use crate::mpd::commands::Song;
use crate::ui::panes::browser::DirOrSong;

pub trait DirStackItem {
    type Item;
    fn as_path(&self) -> &str;
    fn matches(&self, config: &Config, filter: &str) -> bool;
    fn to_list_item(
        &self,
        config: &Config,
        is_marked: bool,
        matches_filter: bool,
        additional_content: Option<String>,
    ) -> Self::Item;
    fn to_list_item_simple(&self, config: &Config) -> Self::Item {
        self.to_list_item(config, false, false, None)
    }
}

impl DirStackItem for DirOrSong {
    type Item = ListItem<'static>;

    fn as_path(&self) -> &str {
        match self {
            DirOrSong::Dir { name, .. } => name,
            DirOrSong::Song(s) => &s.file,
        }
    }

    fn matches(&self, config: &Config, filter: &str) -> bool {
        match self {
            DirOrSong::Dir { name, .. } => if name.is_empty() { "Untitled" } else { name.as_str() }
                .to_lowercase()
                .contains(&filter.to_lowercase()),
            DirOrSong::Song(s) => s.matches(config.theme.browser_song_format.0, filter),
        }
    }

    fn to_list_item(
        &self,
        config: &Config,
        is_marked: bool,
        matches_filter: bool,
        additional_content: Option<String>,
    ) -> Self::Item {
        let symbols = &config.theme.symbols;
        let marker_span = if is_marked {
            Span::styled(symbols.marker, config.theme.highlighted_item_style)
        } else {
            Span::from(" ".repeat(symbols.marker.chars().count()))
        };

        let mut value =
            match self {
                DirOrSong::Dir { name, .. } => Line::from(vec![
                    marker_span,
                    Span::from(format!(
                        "{} {}",
                        symbols.dir,
                        if name.is_empty() { "Untitled" } else { name.as_str() }
                    )),
                ]),
                DirOrSong::Song(s) => {
                    let spans =
                        [marker_span, Span::from(symbols.song), Span::from(" ")].into_iter().chain(
                            config.theme.browser_song_format.0.iter().map(|prop| {
                                Span::from(prop.as_string(Some(s)).unwrap_or_default())
                            }),
                        );
                    Line::from(spans.collect_vec())
                }
            };
        if let Some(content) = additional_content {
            value.push_span(Span::raw(content));
        }
        if matches_filter {
            ListItem::from(value).style(config.theme.highlighted_item_style)
        } else {
            ListItem::from(value)
        }
    }
}

impl DirStackItem for Song {
    type Item = ListItem<'static>;

    fn as_path(&self) -> &str {
        &self.file
    }

    fn matches(&self, config: &Config, filter: &str) -> bool {
        self.matches(config.theme.browser_song_format.0, filter)
    }

    fn to_list_item(
        &self,
        config: &Config,
        is_marked: bool,
        matches_filter: bool,
        additional_content: Option<String>,
    ) -> Self::Item {
        let symbols = &config.theme.symbols;
        let marker_span = if is_marked {
            Span::styled(symbols.marker, config.theme.highlighted_item_style)
        } else {
            Span::from(" ".repeat(symbols.marker.chars().count()))
        };

        let title = self.title_str().to_owned();
        let artist = self.artist_str().to_owned();
        let separator_span = Span::from(" - ");
        let icon_span = Span::from(format!("{} ", symbols.song));
        let mut result =
            vec![marker_span, icon_span, Span::from(artist), separator_span, Span::from(title)];
        if let Some(content) = additional_content {
            result.push(Span::raw(content));
        }
        let mut result = ListItem::new(Line::from(result));
        if matches_filter {
            result = result.style(config.theme.highlighted_item_style);
        }

        result
    }
}

pub trait ScrollingState {
    fn select_scrolling(&mut self, idx: Option<usize>);
    fn get_selected_scrolling(&self) -> Option<usize>;
    fn offset(&self) -> usize;
    fn set_offset(&mut self, value: usize);
}

impl ScrollingState for TableState {
    fn select_scrolling(&mut self, idx: Option<usize>) {
        self.select(idx);
    }

    fn get_selected_scrolling(&self) -> Option<usize> {
        self.selected()
    }

    fn offset(&self) -> usize {
        self.offset()
    }

    fn set_offset(&mut self, value: usize) {
        *self.offset_mut() = value;
    }
}

impl ScrollingState for ListState {
    fn select_scrolling(&mut self, idx: Option<usize>) {
        self.select(idx);
    }

    fn get_selected_scrolling(&self) -> Option<usize> {
        self.selected()
    }

    fn offset(&self) -> usize {
        self.offset()
    }

    fn set_offset(&mut self, value: usize) {
        *self.offset_mut() = value;
    }
}

#[cfg(test)]
impl DirStackItem for String {
    type Item = ListItem<'static>;

    fn as_path(&self) -> &str {
        self
    }

    fn matches(&self, _config: &Config, filter: &str) -> bool {
        self.to_lowercase().contains(&filter.to_lowercase())
    }

    fn to_list_item(
        &self,
        config: &Config,
        is_marked: bool,
        matches_filter: bool,
        _additional_content: Option<String>,
    ) -> Self::Item {
        let symbols = &config.theme.symbols;
        let marker_span = if is_marked {
            Span::styled(symbols.marker, config.theme.highlighted_item_style)
        } else {
            Span::from(" ".repeat(symbols.marker.chars().count()))
        };

        if matches_filter {
            ListItem::new(Line::from(vec![marker_span, Span::from(self.clone())]))
                .style(config.theme.highlighted_item_style)
        } else {
            ListItem::new(Line::from(vec![marker_span, Span::from(self.clone())]))
        }
    }
}
