use std::borrow::Cow;

use itertools::Itertools;
use ratatui::{
    text::{Line, Span},
    widgets::{ListItem, ListState, TableState},
};

mod dir;
mod path;
mod stack;
mod state;
mod walk;
pub use dir::Dir;
pub use path::Path;
use rmpc_mpd::commands::Song;
pub use stack::DirStack;
pub use state::DirState;
pub use walk::WalkDirStackItem;

use super::dir_or_song::DirOrSong;
use crate::{
    config::theme::properties::{Property, SongProperty},
    ctx::Ctx,
    shared::mpd_query::PreviewGroup,
    ui::song_ext::SongExt as _,
};

pub trait DirStackItem {
    fn as_path(&self) -> &str;
    fn is_file(&self) -> bool;
    fn to_file_preview(&self, ctx: &Ctx) -> Vec<PreviewGroup>;
    fn matches(&self, song_format: &[Property<SongProperty>], ctx: &Ctx, filter: &str) -> bool;
    fn to_list_item<'a>(
        &self,
        ctx: &Ctx,
        is_marked: bool,
        matches_filter: bool,
        additional_content: Option<String>,
    ) -> ListItem<'a>;
    fn to_list_item_simple<'a>(&self, ctx: &Ctx) -> ListItem<'a> {
        self.to_list_item(ctx, false, false, None)
    }
    fn format(&self, format: &[Property<SongProperty>], sep: &str, ctx: &Ctx) -> String;
}

impl DirStackItem for DirOrSong {
    fn as_path(&self) -> &str {
        match self {
            DirOrSong::Dir { name, .. } => name,
            DirOrSong::Song(s) => &s.file,
        }
    }

    fn is_file(&self) -> bool {
        match self {
            DirOrSong::Dir { .. } => false,
            DirOrSong::Song(_) => true,
        }
    }

    fn to_file_preview(&self, ctx: &Ctx) -> Vec<PreviewGroup> {
        match self {
            DirOrSong::Dir { .. } => Vec::new(),
            DirOrSong::Song(s) => s.to_file_preview(ctx),
        }
    }

    fn matches(&self, song_format: &[Property<SongProperty>], ctx: &Ctx, filter: &str) -> bool {
        match self {
            DirOrSong::Dir { name, .. } => if name.is_empty() { "Untitled" } else { name.as_str() }
                .to_lowercase()
                .contains(&filter.to_lowercase()),
            DirOrSong::Song(s) => s.matches_formats(song_format, filter, ctx),
        }
    }

    fn to_list_item<'a>(
        &self,
        ctx: &Ctx,
        is_marked: bool,
        matches_filter: bool,
        additional_content: Option<String>,
    ) -> ListItem<'a> {
        match self {
            DirOrSong::Dir { name, playlist: is_playlist, .. } => {
                let config = &ctx.config;
                let marker_span = if is_marked {
                    Span::styled(
                        config.theme.symbols.marker.clone(),
                        config.theme.highlighted_item_style,
                    )
                } else {
                    Span::from(" ".repeat(config.theme.symbols.marker.chars().count()))
                };
                let mut value = Line::from(vec![
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
                ]);

                if let Some(content) = additional_content {
                    value.push_span(Span::raw(content));
                }
                if matches_filter {
                    ListItem::from(value).style(config.theme.highlighted_item_style)
                } else {
                    ListItem::from(value)
                }
            }
            DirOrSong::Song(s) => {
                s.to_list_item(ctx, is_marked, matches_filter, additional_content)
            }
        }
    }

    fn format(&self, format: &[Property<SongProperty>], sep: &str, ctx: &Ctx) -> String {
        match self {
            DirOrSong::Dir { name, .. } => name.clone(),
            DirOrSong::Song(s) => <Song as DirStackItem>::format(s, format, sep, ctx),
        }
    }
}

impl DirStackItem for Song {
    fn as_path(&self) -> &str {
        &self.file
    }

    fn is_file(&self) -> bool {
        true
    }

    fn to_file_preview(&self, ctx: &Ctx) -> Vec<PreviewGroup> {
        let key_style = ctx.config.theme.preview_label_style;
        let group_style = ctx.config.theme.preview_metadata_group_style;
        self.to_preview(key_style, group_style, ctx)
    }

    fn matches(&self, song_format: &[Property<SongProperty>], ctx: &Ctx, filter: &str) -> bool {
        self.matches_formats(song_format, filter, ctx)
    }

    fn to_list_item<'a>(
        &self,
        ctx: &Ctx,
        is_marked: bool,
        matches_filter: bool,
        additional_content: Option<String>,
    ) -> ListItem<'a> {
        let config = &ctx.config;
        let marker_span = if is_marked {
            Span::styled(config.theme.symbols.marker.clone(), config.theme.highlighted_item_style)
        } else {
            Span::from(" ".repeat(config.theme.symbols.marker.chars().count()))
        };

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
                    Some(self),
                    &config.theme.format_tag_separator,
                    config.theme.multiple_tag_resolution_strategy,
                    ctx,
                )
                .unwrap_or_default(),
            )
        }));
        let mut value = Line::from(spans.collect_vec());

        if let Some(content) = additional_content {
            value.push_span(Span::raw(content));
        }
        if matches_filter {
            ListItem::from(value).style(config.theme.highlighted_item_style)
        } else {
            ListItem::from(value)
        }
    }

    fn format(&self, format: &[Property<SongProperty>], sep: &str, ctx: &Ctx) -> String {
        format
            .iter()
            .map(|prop| {
                prop.as_string(
                    Some(self),
                    &ctx.config.theme.format_tag_separator,
                    ctx.config.theme.multiple_tag_resolution_strategy,
                    ctx,
                )
                .unwrap_or_default()
            })
            .join(sep)
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

    fn is_file(&self) -> bool {
        true
    }

    fn to_file_preview(&self, _ctx: &Ctx) -> Vec<PreviewGroup> {
        Vec::new()
    }

    fn matches(&self, _: &[Property<SongProperty>], _ctx: &Ctx, filter: &str) -> bool {
        self.to_lowercase().contains(&filter.to_lowercase())
    }

    fn to_list_item<'a>(
        &self,
        ctx: &Ctx,
        is_marked: bool,
        matches_filter: bool,
        _additional_content: Option<String>,
    ) -> ListItem<'a> {
        let config = &ctx.config;
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

    fn format(&self, _format: &[Property<SongProperty>], _sep: &str, _ctx: &Ctx) -> String {
        self.clone()
    }
}
