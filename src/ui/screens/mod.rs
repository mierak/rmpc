use std::borrow::Cow;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent};
use either::Either;
use ratatui::{
    prelude::Rect,
    style::Style,
    text::{Line, Span},
    widgets::ListItem,
    Frame,
};
use strum::{Display, EnumIter, VariantNames};

use crate::{
    config::{
        keys::{CommonAction, ToDescription},
        theme::properties::{Property, PropertyKind, PropertyKindOrText, SongProperty, StatusProperty, WidgetProperty},
        Config,
    },
    mpd::{
        commands::{status::OnOffOneshot, volume::Bound, Song, Status},
        mpd_client::MpdClient,
    },
};

use super::{
    utils::dirstack::{DirStack, DirStackItem},
    widgets::volume::Volume,
    DurationExt, KeyHandleResultInternal, UiEvent,
};

pub mod albums;
pub mod artists;
pub mod directories;
#[cfg(debug_assertions)]
pub mod logs;
pub mod playlists;
pub mod queue;
pub mod search;

#[derive(Debug, Display, VariantNames, Default, Clone, Copy, EnumIter, PartialEq)]
pub enum Screens {
    #[default]
    Queue,
    #[cfg(debug_assertions)]
    Logs,
    Directories,
    Artists,
    Albums,
    Playlists,
    Search,
}

#[allow(unused_variables)]
pub(super) trait Screen {
    type Actions: ToDescription;
    fn render(&mut self, frame: &mut Frame, area: Rect, status: &Status, config: &Config) -> Result<()>;

    /// For any cleanup operations, ran when the screen hides
    fn on_hide(&mut self, client: &mut impl MpdClient, status: &mut Status, config: &Config) -> Result<()> {
        Ok(())
    }

    /// For work that needs to be done BEFORE the first render
    fn before_show(&mut self, client: &mut impl MpdClient, status: &mut Status, config: &Config) -> Result<()> {
        Ok(())
    }

    /// Used to keep the current state but refresh data
    fn on_event(
        &mut self,
        event: &mut UiEvent,
        client: &mut impl MpdClient,
        status: &mut Status,
        config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        Ok(KeyHandleResultInternal::SkipRender)
    }

    fn handle_action(
        &mut self,
        event: KeyEvent,
        client: &mut impl MpdClient,
        status: &mut Status,
        config: &Config,
    ) -> Result<KeyHandleResultInternal>;
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
            Screens::Playlists => Screens::Search,
            Screens::Search => Screens::Queue,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Screens::Queue => Screens::Search,
            Screens::Search => Screens::Playlists,
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
        config::theme::SymbolsConfig,
        mpd::commands::{lsinfo::FileOrDir, Song},
    };

    impl Song {
        pub(crate) fn to_preview(&self, _symbols: &SymbolsConfig) -> impl Iterator<Item = ListItem<'static>> {
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

            r.into_iter().map(ListItem::new)
        }
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum DirOrSong {
        Dir(String),
        Song(String),
    }

    impl DirOrSong {
        pub fn value(&self) -> &str {
            match self {
                DirOrSong::Dir(v) => v,
                DirOrSong::Song(v) => v,
            }
        }
    }

    impl std::cmp::Ord for DirOrSong {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            match (self, other) {
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

    impl From<FileOrDir> for DirOrSong {
        fn from(value: FileOrDir) -> Self {
            match value {
                FileOrDir::Dir(dir) => DirOrSong::Dir(dir.path),
                FileOrDir::File(song) => DirOrSong::Song(song.file),
            }
        }
    }
}

impl Song {
    pub fn title_str(&self) -> &str {
        self.title.as_ref().map_or("Untitled", |v| v.as_str())
    }

    pub fn artist_str(&self) -> &str {
        self.artist.as_ref().map_or("Untitled", |v| v.as_str())
    }

    pub fn matches(&self, formats: &[&Property<'static, SongProperty>], filter: &str, ignore_case: bool) -> bool {
        for format in formats {
            let match_found = match &format.kind {
                PropertyKindOrText::Text(value) => value.matches(filter, ignore_case),
                PropertyKindOrText::Property(p) => match p {
                    SongProperty::Filename => self.file.matches(filter, ignore_case),
                    SongProperty::Title => self.title.as_ref().map_or_else(
                        || format.default.is_some_and(|f| self.matches(&[f], filter, ignore_case)),
                        |v| v.matches(filter, ignore_case),
                    ),
                    SongProperty::Artist => self.artist.as_ref().map_or_else(
                        || format.default.is_some_and(|f| self.matches(&[f], filter, ignore_case)),
                        |v| v.matches(filter, ignore_case),
                    ),
                    SongProperty::Album => self.album.as_ref().map_or_else(
                        || format.default.is_some_and(|f| self.matches(&[f], filter, ignore_case)),
                        |v| v.matches(filter, ignore_case),
                    ),
                    SongProperty::Duration => self.duration.as_ref().map_or_else(
                        || format.default.is_some_and(|f| self.matches(&[f], filter, ignore_case)),
                        |duration| duration.to_string().matches(filter, ignore_case),
                    ),
                    SongProperty::Other(name) => self.others.get(*name).map_or_else(
                        || format.default.is_some_and(|f| self.matches(&[f], filter, ignore_case)),
                        |v| v.matches(filter, ignore_case),
                    ),
                },
            };
            if match_found {
                return true;
            }
        }
        return false;
    }

    fn default_as_line_ellipsized<'song>(
        &'song self,
        format: &'static Property<'static, SongProperty>,
        max_len: usize,
    ) -> Line<'song> {
        format
            .default
            .as_ref()
            .map_or(Line::default(), |f| self.as_line_ellipsized(f, max_len))
    }

    pub fn as_line_ellipsized<'song>(
        &'song self,
        format: &'static Property<'static, SongProperty>,
        max_len: usize,
    ) -> Line<'song> {
        let style = format.style.unwrap_or_default();
        match &format.kind {
            PropertyKindOrText::Text(value) => Line::styled(value.ellipsize(max_len).to_string(), style),
            PropertyKindOrText::Property(s) => match s {
                SongProperty::Filename => Line::styled(self.file.ellipsize(max_len).to_string(), style),
                SongProperty::Title => self.title.as_ref().map_or_else(
                    || self.default_as_line_ellipsized(format, max_len),
                    |v| Line::styled(v.ellipsize(max_len), style),
                ),
                SongProperty::Artist => self.artist.as_ref().map_or_else(
                    || self.default_as_line_ellipsized(format, max_len),
                    |v| Line::styled(v.ellipsize(max_len), style),
                ),
                SongProperty::Album => self.album.as_ref().map_or_else(
                    || self.default_as_line_ellipsized(format, max_len),
                    |v| Line::styled(v.ellipsize(max_len), style),
                ),
                SongProperty::Duration => self.duration.as_ref().map_or_else(
                    || self.default_as_line_ellipsized(format, max_len),
                    |v| Line::styled(v.to_string(), style),
                ),
                SongProperty::Other(name) => self.others.get(*name).map_or_else(
                    || self.default_as_line_ellipsized(format, max_len),
                    |v| Line::styled(v.ellipsize(max_len), style),
                ),
            },
        }
    }
}

impl Property<'static, PropertyKind> {
    fn default_as_span<'song: 's, 's>(
        &self,
        song: Option<&'song Song>,
        status: &'song Status,
    ) -> Either<Span<'s>, Vec<Span<'s>>> {
        self.default
            .as_ref()
            .map_or(Either::Right(Vec::default()), |p| p.as_span(song, status))
    }

    pub fn as_span<'song: 's, 's>(
        &'s self,
        song: Option<&'song Song>,
        status: &'song Status,
    ) -> Either<Span<'s>, Vec<Span<'s>>> {
        let style = self.style.unwrap_or_default();
        match &self.kind {
            PropertyKindOrText::Text(value) => Either::Left(Span::styled(value.as_str(), style)),
            PropertyKindOrText::Property(PropertyKind::Song(s)) => match (s, song) {
                (SongProperty::Filename, None) => Either::Left(Span::styled("", Style::default())), // cannot happen
                (SongProperty::Title, None) => self.default_as_span(song, status),
                (SongProperty::Artist, None) => self.default_as_span(song, status),
                (SongProperty::Album, None) => self.default_as_span(song, status),
                (SongProperty::Duration, None) => self.default_as_span(song, status),
                (SongProperty::Other { .. }, None) => self.default_as_span(song, status),
                (SongProperty::Filename, Some(s)) => Either::Left(Span::styled(s.file.as_str(), style)),
                (SongProperty::Title, Some(s)) => s.title.as_ref().map_or_else(
                    || self.default_as_span(song, status),
                    |v| Either::Left(Span::styled(v.as_str(), style)),
                ),
                (SongProperty::Artist, Some(s)) => s.artist.as_ref().map_or_else(
                    || self.default_as_span(song, status),
                    |v| Either::Left(Span::styled(v.as_str(), style)),
                ),
                (SongProperty::Album, Some(s)) => s.album.as_ref().map_or_else(
                    || self.default_as_span(song, status),
                    |v| Either::Left(Span::styled(v.as_str(), style)),
                ),
                (SongProperty::Duration, Some(s)) => s.duration.as_ref().map_or_else(
                    || self.default_as_span(song, status),
                    |v| Either::Left(Span::styled(v.to_string(), style)),
                ),
                (SongProperty::Other(name), Some(s)) => s.others.get(*name).map_or_else(
                    || self.default_as_span(song, status),
                    |v| Either::Left(Span::styled(v.as_str(), style)),
                ),
            },
            PropertyKindOrText::Property(PropertyKind::Status(s)) => match s {
                StatusProperty::State => Either::Left(Span::styled(status.state.as_ref(), style)),
                StatusProperty::Duration => Either::Left(Span::styled(status.duration.to_string(), style)),
                StatusProperty::Elapsed => Either::Left(Span::styled(status.elapsed.to_string(), style)),
                StatusProperty::Volume => Either::Left(Span::styled(status.volume.value().to_string(), style)),
                StatusProperty::Repeat => Either::Left(Span::styled(if status.repeat { "On" } else { "Off" }, style)),
                StatusProperty::Random => Either::Left(Span::styled(if status.random { "On" } else { "Off" }, style)),
                StatusProperty::Consume => Either::Left(Span::styled(status.consume.to_string(), style)),
                StatusProperty::Single => Either::Left(Span::styled(status.single.to_string(), style)),
                StatusProperty::Bitrate => status.bitrate.as_ref().map_or_else(
                    || self.default_as_span(song, status),
                    |v| Either::Left(Span::styled(v.to_string(), Style::default())),
                ),
                StatusProperty::Crossfade => status.xfade.as_ref().map_or_else(
                    || self.default_as_span(song, status),
                    |v| Either::Left(Span::styled(v.to_string(), Style::default())),
                ),
            },
            PropertyKindOrText::Property(PropertyKind::Widget(w)) => match w {
                WidgetProperty::Volume => Either::Left(Span::styled(Volume::get_str(*status.volume.value()), style)),
                WidgetProperty::States {
                    active_style,
                    separator_style,
                } => {
                    let separator = Span::styled(" / ", *separator_style);
                    Either::Right(vec![
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
                    ])
                }
            },
        }
    }
}

pub(crate) trait StringExt {
    fn file_name(&self) -> &str;
    fn ellipsize(&self, max_len: usize) -> Cow<str>;
}

impl StringExt for String {
    fn file_name(&self) -> &str {
        self.rsplit('/')
            .next()
            .map_or(self, |v| v.rsplit_once('.').map_or(v, |v| v.0))
    }

    fn ellipsize(&self, max_len: usize) -> Cow<str> {
        if self.chars().count() > max_len {
            Cow::Owned(format!(
                "{}...",
                self.chars().take(max_len.saturating_sub(4)).collect::<String>()
            ))
        } else {
            Cow::Borrowed(self)
        }
    }
}

enum MoveDirection {
    Up,
    Down,
}

#[allow(unused)]
trait BrowserScreen<T: DirStackItem + std::fmt::Debug>: Screen {
    fn stack(&self) -> &DirStack<T>;
    fn stack_mut(&mut self) -> &mut DirStack<T>;
    fn set_filter_input_mode_active(&mut self, active: bool);
    fn is_filter_input_mode_active(&self) -> bool;
    fn next(&mut self, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal>;
    fn move_selected(
        &mut self,
        direction: MoveDirection,
        client: &mut impl MpdClient,
    ) -> Result<KeyHandleResultInternal> {
        Ok(KeyHandleResultInternal::SkipRender)
    }
    fn prepare_preview(
        &mut self,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<Option<Vec<ListItem<'static>>>>;
    fn add(&self, item: &T, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal>;
    fn delete(&self, item: &T, index: usize, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        Ok(KeyHandleResultInternal::SkipRender)
    }
    fn rename(&self, item: &T, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        Ok(KeyHandleResultInternal::SkipRender)
    }
    fn handle_filter_input(
        &mut self,
        event: KeyEvent,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        match config.keybinds.navigation.get(&event.into()) {
            Some(CommonAction::Close) => {
                self.set_filter_input_mode_active(false);
                self.stack_mut().current_mut().filter = None;
                let preview = self.prepare_preview(client, config)?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            Some(CommonAction::Confirm) => {
                self.set_filter_input_mode_active(false);
                self.stack_mut().current_mut().jump_next_matching();
                let preview = self.prepare_preview(client, config)?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            _ => match event.code {
                KeyCode::Char(c) => {
                    if let Some(ref mut f) = self.stack_mut().current_mut().filter {
                        f.push(c);
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Backspace => {
                    if let Some(ref mut f) = self.stack_mut().current_mut().filter {
                        f.pop();
                    };
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                _ => Ok(KeyHandleResultInternal::SkipRender),
            },
        }
    }

    fn handle_common_action(
        &mut self,
        action: CommonAction,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        match action {
            CommonAction::Up => {
                self.stack_mut().current_mut().prev();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Down => {
                self.stack_mut().current_mut().next();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::MoveUp => {
                let res = self.move_selected(MoveDirection::Up, client)?;
                Ok(res)
            }
            CommonAction::MoveDown => {
                let res = self.move_selected(MoveDirection::Down, client)?;
                Ok(res)
            }
            CommonAction::DownHalf => {
                self.stack_mut().current_mut().next_half_viewport();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::UpHalf => {
                self.stack_mut().current_mut().prev_half_viewport();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Bottom => {
                self.stack_mut().current_mut().last();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Top => {
                self.stack_mut().current_mut().first();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Right => {
                let res = self.next(client)?;
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(res)
            }
            CommonAction::Left => {
                self.stack_mut().pop();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::EnterSearch => {
                self.set_filter_input_mode_active(true);
                self.stack_mut().current_mut().filter = Some(String::new());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::NextResult => {
                self.stack_mut().current_mut().jump_next_matching();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::PreviousResult => {
                self.stack_mut().current_mut().jump_previous_matching();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Select => {
                self.stack_mut().current_mut().toggle_mark_selected();
                self.stack_mut().current_mut().next();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Add if !self.stack().current().marked().is_empty() => {
                for idx in self.stack().current().marked().iter().rev() {
                    let item = &self.stack().current().items[*idx];
                    self.add(item, client)?;
                }
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Add => {
                if let Some(item) = self.stack().current().selected() {
                    self.add(item, client)
                } else {
                    Ok(KeyHandleResultInternal::SkipRender)
                }
            }
            CommonAction::Delete if !self.stack().current().marked().is_empty() => {
                for idx in self.stack().current().marked().iter().rev() {
                    let item = &self.stack().current().items[*idx];
                    self.delete(item, *idx, client)?;
                }
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Delete => {
                if let Some((index, item)) = self.stack().current().selected_with_idx() {
                    self.delete(item, index, client)?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                } else {
                    Ok(KeyHandleResultInternal::SkipRender)
                }
            }
            CommonAction::Rename => {
                if let Some(item) = self.stack().current().selected() {
                    self.rename(item, client)
                } else {
                    Ok(KeyHandleResultInternal::SkipRender)
                }
            }
            CommonAction::FocusInput => Ok(KeyHandleResultInternal::SkipRender),
            CommonAction::Close => Ok(KeyHandleResultInternal::SkipRender), // todo out?
            CommonAction::Confirm => Ok(KeyHandleResultInternal::SkipRender), // todo next?
        }
    }
}
