use std::borrow::Cow;

use itertools::Itertools;
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

use super::dir_or_song::DirOrSong;
use crate::{config::Config, mpd::commands::Song};

pub trait DirStackItem {
    fn as_path(&self) -> &str;
    fn matches(&self, config: &Config, filter: &str) -> bool;
    fn to_list_item<'a>(
        &self,
        config: &Config,
        is_marked: bool,
        matches_filter: bool,
        additional_content: Option<String>,
    ) -> ListItem<'a>;
    fn to_list_item_simple<'a>(&self, config: &Config) -> ListItem<'a> {
        self.to_list_item(config, false, false, None)
    }
}

impl DirStackItem for DirOrSong {
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
            DirOrSong::Song(s) => s.matches(config.theme.browser_song_format.0.as_slice(), filter),
        }
    }

    fn to_list_item<'a>(
        &self,
        config: &Config,
        is_marked: bool,
        matches_filter: bool,
        additional_content: Option<String>,
    ) -> ListItem<'a> {
        let marker_span = if is_marked {
            Span::styled(config.theme.symbols.marker.clone(), config.theme.highlighted_item_style)
        } else {
            Span::from(" ".repeat(config.theme.symbols.marker.chars().count()))
        };

        let mut value = match self {
            DirOrSong::Dir { name, playlist: is_playlist, .. } => Line::from(vec![
                marker_span,
                if *is_playlist {
                    Span::styled(
                        config.theme.symbols.playlist.clone(),
                        config.theme.symbols.playlist_style.unwrap_or_default(),
                    )
                } else {
                    Span::styled(
                        config.theme.symbols.dir.clone(),
                        config.theme.symbols.dir_style.unwrap_or_default(),
                    )
                },
                Span::from(" "),
                Span::from(if name.is_empty() {
                    Cow::Borrowed("Untitled")
                } else {
                    Cow::Owned(name.to_owned())
                }),
            ]),
            DirOrSong::Song(s) => {
                let spans = [
                    marker_span,
                    Span::styled(
                        config.theme.symbols.song.clone(),
                        config.theme.symbols.song_style.unwrap_or_default(),
                    ),
                    Span::from(" "),
                ]
                .into_iter()
                .chain(config.theme.browser_song_format.0.iter().map(|prop| {
                    Span::from(
                        prop.as_string(
                            Some(s),
                            &config.theme.format_tag_separator,
                            config.theme.multiple_tag_resolution_strategy,
                        )
                        .unwrap_or_default(),
                    )
                }));
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
    fn as_path(&self) -> &str {
        &self.file
    }

    fn matches(&self, config: &Config, filter: &str) -> bool {
        self.matches(config.theme.browser_song_format.0.as_slice(), filter)
    }

    fn to_list_item<'a>(
        &self,
        config: &Config,
        is_marked: bool,
        matches_filter: bool,
        additional_content: Option<String>,
    ) -> ListItem<'a> {
        let marker_span = if is_marked {
            Span::styled(config.theme.symbols.marker.clone(), config.theme.highlighted_item_style)
        } else {
            Span::from(" ".repeat(config.theme.symbols.marker.chars().count()))
        };

        let title = self.title_str(&config.theme.format_tag_separator).into_owned();
        let artist = self.artist_str(&config.theme.format_tag_separator).into_owned();
        let separator_span = Span::from(" - ");
        let icon_span = Span::styled(
            format!("{} ", config.theme.symbols.song),
            config.theme.symbols.song_style.unwrap_or_default(),
        );
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
    fn as_path(&self) -> &str {
        self
    }

    fn matches(&self, _config: &Config, filter: &str) -> bool {
        self.to_lowercase().contains(&filter.to_lowercase())
    }

    fn to_list_item<'a>(
        &self,
        config: &Config,
        is_marked: bool,
        matches_filter: bool,
        _additional_content: Option<String>,
    ) -> ListItem<'a> {
        let marker_span = if is_marked {
            Span::styled(config.theme.symbols.marker.clone(), config.theme.highlighted_item_style)
        } else {
            Span::from(" ".repeat(config.theme.symbols.marker.chars().count()))
        };

        if matches_filter {
            ListItem::new(Line::from(vec![marker_span, Span::from(self.clone())]))
                .style(config.theme.highlighted_item_style)
        } else {
            ListItem::new(Line::from(vec![marker_span, Span::from(self.clone())]))
        }
    }
}
