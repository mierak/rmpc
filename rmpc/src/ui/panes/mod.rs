use std::{
    collections::{HashMap, VecDeque},
    time::Duration,
};

use album_art::AlbumArtPane;
use albums::AlbumsPane;
use anyhow::{Context, Result};
use cava::CavaPane;
use directories::DirectoriesPane;
use either::Either;
use header::HeaderPane;
use lyrics::LyricsPane;
use playlists::PlaylistsPane;
use progress_bar::ProgressBarPane;
use property::PropertyPane;
use queue::QueuePane;
use ratatui::{Frame, layout::Layout, prelude::Rect, style::Color, text::Span, widgets::Block};
use rmpc_mpd::{
    commands::{Song, State, status::OnOffOneshot, volume::Bound},
    filter::Tag,
};
use search::SearchPane;
use strum::{Display, IntoDiscriminant};
use tabs::TabsPane;
use tag_browser::TagBrowserPane;
use volume::VolumePane;

#[cfg(debug_assertions)]
use self::{frame_count::FrameCountPane, logs::LogsPane};
use super::{
    UiEvent,
    widgets::{scan_status::ScanStatus, volume::Volume},
};
use crate::{
    MpdQueryResult,
    config::{
        tabs::{Pane as ConfigPane, PaneType, SizedPaneOrSplit},
        theme::{
            TagResolutionStrategy,
            properties::{
                Property,
                PropertyKind,
                PropertyKindOrText,
                SongProperty,
                StatusProperty,
                Transform,
                WidgetProperty,
            },
        },
    },
    ctx::Ctx,
    shared::{
        ext::{duration::DurationExt, num::NumExt, span::SpanExt},
        keys::ActionEvent,
        mouse_event::MouseEvent,
    },
    ui::{
        input::InputResultEvent,
        panes::{empty::EmptyPane, queue_header::QueueHeaderPane},
        song_ext::SongExt as _,
        widgets::header::PropertyTemplates,
    },
};

pub mod album_art;
pub mod albums;
pub mod cava;
pub mod directories;
pub mod empty;
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
pub mod queue_header;
pub mod search;
pub mod tabs;
pub mod tag_browser;
pub mod volume;

#[derive(Debug, Display, strum::EnumDiscriminants)]
pub enum Panes<'pane_ref, 'pane> {
    Queue(&'pane_ref mut QueuePane),
    QueueHeader(&'pane_ref mut QueueHeaderPane),
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
    Cava(&'pane_ref mut CavaPane),
    Empty(&'pane_ref mut EmptyPane),
}

pub trait BoxedPane: Pane + std::fmt::Debug {}

impl<P: Pane + std::fmt::Debug> BoxedPane for P {}

#[derive(Debug)]
pub struct PaneContainer<'panes> {
    pub queue: QueuePane,
    pub queue_header: QueueHeaderPane,
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
    pub cava: CavaPane,
    #[cfg(debug_assertions)]
    pub frame_count: FrameCountPane,
    pub empty: EmptyPane,
    pub others: HashMap<PaneType, Box<dyn BoxedPane>>,
}

impl<'panes> PaneContainer<'panes> {
    pub fn new(ctx: &Ctx) -> Result<Self> {
        Ok(Self {
            queue: QueuePane::new(ctx),
            queue_header: QueueHeaderPane::new(ctx),
            #[cfg(debug_assertions)]
            logs: LogsPane::new(),
            directories: DirectoriesPane::new(ctx),
            albums: AlbumsPane::new(ctx),
            artists: TagBrowserPane::new(Tag::Artist, PaneType::Artists, None, ctx),
            album_artists: TagBrowserPane::new(Tag::AlbumArtist, PaneType::AlbumArtists, None, ctx),
            playlists: PlaylistsPane::new(ctx),
            search: SearchPane::new(ctx),
            album_art: AlbumArtPane::new(ctx),
            lyrics: LyricsPane::new(ctx),
            progress_bar: ProgressBarPane::new(),
            header: HeaderPane::new(),
            tabs: TabsPane::new(ctx)?,
            cava: CavaPane::new(ctx),
            #[cfg(debug_assertions)]
            frame_count: FrameCountPane::new(),
            empty: EmptyPane,
            others: Self::init_other_panes(ctx).collect(),
        })
    }

    pub fn init_other_panes(
        ctx: &Ctx,
    ) -> impl Iterator<Item = (PaneType, Box<dyn BoxedPane>)> + use<'_> {
        ctx.config
            .tabs
            .tabs
            .iter()
            .flat_map(|(_name, tab)| tab.panes.panes_iter())
            .chain(ctx.config.theme.layout.panes_iter())
            .filter_map(|pane| match &pane.pane {
                PaneType::Browser { root_tag, separator } => Some((
                    pane.pane.clone(),
                    Box::new(TagBrowserPane::new(
                        Tag::Custom(root_tag.clone()),
                        pane.pane.clone(),
                        separator.clone(),
                        ctx,
                    )) as Box<dyn BoxedPane>,
                )),
                PaneType::Volume { kind } => Some((
                    pane.pane.clone(),
                    Box::new(VolumePane::new(kind.clone())) as Box<dyn BoxedPane>,
                )),
                _ => None,
            })
    }

    pub fn get_mut<'pane_ref, 'pane_type_ref: 'pane_ref>(
        &'pane_ref mut self,
        pane: &'pane_type_ref PaneType,
        ctx: &Ctx,
    ) -> Result<Panes<'pane_ref, 'panes>> {
        match pane {
            PaneType::Queue => Ok(Panes::Queue(&mut self.queue)),
            PaneType::QueueHeader() => Ok(Panes::QueueHeader(&mut self.queue_header)),
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
            PaneType::Property { content, align, scroll_speed } => Ok(Panes::Property(
                PropertyPane::<'pane_type_ref>::new(content, *align, (*scroll_speed).into(), ctx),
            )),
            p @ PaneType::Volume { .. } => Ok(Panes::Others(
                self.others
                    .get_mut(pane)
                    .with_context(|| format!("expected pane to be defined {p:?}"))?,
            )),
            p @ PaneType::Browser { .. } => Ok(Panes::Others(
                self.others
                    .get_mut(pane)
                    .with_context(|| format!("expected pane to be defined {p:?}"))?,
            )),
            PaneType::Cava => Ok(Panes::Cava(&mut self.cava)),
            PaneType::Empty => Ok(Panes::Empty(&mut self.empty)),
        }
    }
}

macro_rules! pane_call {
    ($screen:ident, $fn:ident($($param:expr),+)) => {
        match &mut $screen {
            Panes::Queue(s) => s.$fn($($param),+),
            Panes::QueueHeader(s) => s.$fn($($param),+),
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
            Panes::Cava(s) => s.$fn($($param),+),
            Panes::Empty(s) => s.$fn($($param),+),
        }
    }
}
pub(crate) use pane_call;

#[allow(unused_variables)]
pub(crate) trait Pane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()>;

    /// For any cleanup operations, ran when the screen hides
    fn on_hide(&mut self, ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    /// For work that needs to be done BEFORE the first render
    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    /// Used to keep the current state but refresh data
    fn on_event(&mut self, event: &mut UiEvent, is_visible: bool, ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn handle_insert_mode(&mut self, kind: InputResultEvent, ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }

    fn handle_action(&mut self, event: &mut ActionEvent, ctx: &mut Ctx) -> Result<()>;

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn on_query_finished(
        &mut self,
        id: &'static str,
        data: MpdQueryResult,
        is_visible: bool,
        ctx: &Ctx,
    ) -> Result<()> {
        Ok(())
    }

    fn calculate_areas(&mut self, area: Rect, ctx: &Ctx) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, area: Rect, ctx: &Ctx) -> Result<()> {
        Ok(())
    }
}

impl Property<SongProperty> {
    fn default(
        &self,
        song: Option<&Song>,
        tag_separator: &str,
        strategy: TagResolutionStrategy,
        ctx: &Ctx,
    ) -> Option<String> {
        self.default.as_ref().and_then(|p| p.as_string(song, tag_separator, strategy, ctx))
    }

    pub fn as_string(
        &self,
        song: Option<&Song>,
        tag_separator: &str,
        strategy: TagResolutionStrategy,
        ctx: &Ctx,
    ) -> Option<String> {
        match &self.kind {
            PropertyKindOrText::Text(value) => Some((*value).clone()),
            PropertyKindOrText::Sticker(key) => song
                .and_then(|s| ctx.song_stickers(&s.file))
                .and_then(|s| s.get(key))
                .cloned()
                .or_else(|| self.default(song, tag_separator, strategy, ctx)),
            PropertyKindOrText::Property(property) => {
                if let Some(song) = song {
                    song.format(property, tag_separator, strategy).map_or_else(
                        || self.default(Some(song), tag_separator, strategy, ctx),
                        |v| Some(v.into_owned()),
                    )
                } else {
                    self.default(song, tag_separator, strategy, ctx)
                }
            }
            PropertyKindOrText::Group(group) => {
                let mut buf = String::new();
                for format in group {
                    if let Some(res) = format.as_string(song, tag_separator, strategy, ctx) {
                        buf.push_str(&res);
                    } else {
                        return self
                            .default
                            .as_ref()
                            .and_then(|d| d.as_string(song, tag_separator, strategy, ctx));
                    }
                }

                Some(buf)
            }
            PropertyKindOrText::Transform(Transform::Replace { content, replacements }) => content
                .as_string(song, tag_separator, strategy, ctx)
                .and_then(|result| {
                    if let Some(replacement) = replacements.get(&result) {
                        return replacement.as_string(song, tag_separator, strategy, ctx).or_else(
                            || {
                                replacement
                                    .default
                                    .as_ref()
                                    .and_then(|d| d.as_string(song, tag_separator, strategy, ctx))
                            },
                        );
                    }

                    Some(result)
                })
                .or_else(|| {
                    self.default
                        .as_ref()
                        .and_then(|d| d.as_string(song, tag_separator, strategy, ctx))
                }),
            PropertyKindOrText::Transform(Transform::Truncate { content, length, from_start }) => {
                content
                    .as_string(song, tag_separator, strategy, ctx)
                    .map(|mut result| {
                        if *from_start {
                            result.truncate_start(*length);
                        } else {
                            result.truncate_end(*length);
                        }
                        result
                    })
                    .or_else(|| {
                        self.default
                            .as_ref()
                            .and_then(|d| d.as_string(song, tag_separator, strategy, ctx))
                    })
            }
        }
    }
}

impl Property<PropertyKind> {
    fn default_as_span<'song: 's, 's>(
        &'s self,
        song: Option<&'song Song>,
        ctx: &'song Ctx,
        tag_separator: &str,
        strategy: TagResolutionStrategy,
    ) -> Option<Either<Span<'s>, Vec<Span<'s>>>> {
        self.default.as_ref().and_then(|p| p.as_span(song, ctx, tag_separator, strategy))
    }

    pub fn as_span<'song: 's, 's>(
        &'s self,
        song: Option<&'song Song>,
        ctx: &'song Ctx,
        tag_separator: &str,
        strategy: TagResolutionStrategy,
    ) -> Option<Either<Span<'s>, Vec<Span<'s>>>> {
        let style = self.style.unwrap_or_default();
        let status = &ctx.status;
        match &self.kind {
            PropertyKindOrText::Text(value) => Some(Either::Left(Span::styled(value, style))),
            PropertyKindOrText::Sticker(key) => {
                if let Some(sticker) =
                    song.and_then(|s| ctx.song_stickers(&s.file)).and_then(|s| s.get(key))
                {
                    Some(Either::Left(Span::styled(sticker, style)))
                } else {
                    self.default_as_span(song, ctx, tag_separator, strategy)
                }
            }
            PropertyKindOrText::Property(PropertyKind::Song(property)) => {
                if let Some(song) = song {
                    song.format(property, tag_separator, strategy).map_or_else(
                        || self.default_as_span(Some(song), ctx, tag_separator, strategy),
                        |s| Some(Either::Left(Span::styled(s, style))),
                    )
                } else {
                    self.default_as_span(song, ctx, tag_separator, strategy)
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
                StatusProperty::Partition => {
                    Some(Either::Left(Span::styled(&status.partition, style)))
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
                    || self.default_as_span(song, ctx, tag_separator, strategy),
                    |v| Some(Either::Left(Span::styled(v.to_string(), style))),
                ),
                StatusProperty::Crossfade => status.xfade.as_ref().map_or_else(
                    || self.default_as_span(song, ctx, tag_separator, strategy),
                    |v| Some(Either::Left(Span::styled(v.to_string(), style))),
                ),
                StatusProperty::QueueLength { thousands_separator } => {
                    Some(Either::Left(Span::styled(
                        ctx.queue.len().with_thousands_separator(thousands_separator),
                        style,
                    )))
                }
                StatusProperty::QueueTimeTotal { separator } => {
                    let formatted = match separator {
                        Some(sep) => ctx.cached_queue_time_total.format_to_duration(sep),
                        None => ctx.cached_queue_time_total.to_string(),
                    };
                    Some(Either::Left(Span::styled(formatted, style)))
                }
                StatusProperty::QueueTimeRemaining { separator } => {
                    let remaining_time = ctx.find_current_song_in_queue().map_or(
                        Duration::default(),
                        |(current_song_idx, current_song)| {
                            let total_remaining: Duration = ctx
                                .queue
                                .iter()
                                .skip(current_song_idx)
                                .filter_map(|s| s.duration)
                                .sum();
                            if current_song.duration.is_some() {
                                total_remaining.saturating_sub(ctx.status.elapsed)
                            } else {
                                total_remaining
                            }
                        },
                    );
                    let formatted = match separator {
                        Some(sep) => remaining_time.format_to_duration(sep),
                        None => remaining_time.to_string(),
                    };
                    Some(Either::Left(Span::styled(formatted, style)))
                }
                StatusProperty::ActiveTab => {
                    Some(Either::Left(Span::styled(ctx.active_tab.0.as_ref(), style)))
                }
                StatusProperty::InputBuffer() => {
                    Some(Either::Left(Span::styled(ctx.key_resolver.buffer_to_string(), style)))
                }
                StatusProperty::InputMode() => Some(Either::Left(Span::styled(
                    ctx.input.mode().discriminant().to_string(),
                    style,
                ))),
                StatusProperty::SampleRate() => {
                    status.samplerate().map(|v| Either::Left(Span::styled(v.to_string(), style)))
                }
                StatusProperty::Bits() => {
                    status.bits().map(|v| Either::Left(Span::styled(v.to_string(), style)))
                }
                StatusProperty::Channels() => {
                    status.channels().map(|v| Either::Left(Span::styled(v.to_string(), style)))
                }
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
                WidgetProperty::ScanStatus => ctx
                    .db_update_start
                    .map(|update_start| {
                        Either::Left(Span::styled(
                            ScanStatus::new(Some(update_start))
                                .get_str()
                                .unwrap_or_default()
                                .to_string(),
                            style,
                        ))
                    })
                    .or_else(|| self.default_as_span(song, ctx, tag_separator, strategy)),
            },
            PropertyKindOrText::Group(group) => {
                let mut buf = Vec::new();
                for format in group {
                    match format.as_span(song, ctx, tag_separator, strategy) {
                        Some(Either::Left(span)) => buf.push(span),
                        Some(Either::Right(spans)) => buf.extend(spans),
                        None => return self.default_as_span(song, ctx, tag_separator, strategy),
                    }
                }

                Some(Either::Right(buf))
            }
            PropertyKindOrText::Transform(Transform::Replace { content, replacements }) => {
                match content.as_span(song, ctx, tag_separator, strategy) {
                    Some(Either::Left(span)) => {
                        if let Some(replacement) = replacements.get(span.content.as_ref()) {
                            return replacement
                                .as_span(song, ctx, tag_separator, strategy)
                                .or_else(|| {
                                    replacement.default_as_span(song, ctx, tag_separator, strategy)
                                });
                        }

                        Some(Either::Left(span))
                    }
                    Some(Either::Right(spans)) => {
                        let mut content = String::new();
                        for span in &spans {
                            content.push_str(span.content.as_ref());
                        }

                        if let Some(replacement) = replacements.get(&content) {
                            return replacement
                                .as_span(song, ctx, tag_separator, strategy)
                                .or_else(|| {
                                    replacement.default_as_span(song, ctx, tag_separator, strategy)
                                });
                        }

                        Some(Either::Right(spans))
                    }
                    None => self.default_as_span(song, ctx, tag_separator, strategy),
                }
            }
            PropertyKindOrText::Transform(Transform::Truncate { content, length, from_start }) => {
                let truncate_fn =
                    if *from_start { Span::truncate_start } else { Span::truncate_end };
                match content.as_span(song, ctx, tag_separator, strategy) {
                    Some(Either::Left(mut span)) => {
                        truncate_fn(&mut span, *length);
                        Some(Either::Left(span))
                    }
                    Some(Either::Right(mut spans)) => {
                        let mut buf = VecDeque::new();
                        let mut remaining_len = *length;
                        let push_fn =
                            if *from_start { VecDeque::push_front } else { VecDeque::push_back };
                        let spans_len = spans.len();

                        for i in 0..spans.len() {
                            if remaining_len == 0 {
                                break;
                            }
                            let i = if *from_start { spans_len - 1 - i } else { i };
                            let mut span = std::mem::take(&mut spans[i]);

                            let remaining = truncate_fn(&mut span, remaining_len);
                            push_fn(&mut buf, span);
                            remaining_len = remaining_len.saturating_sub(remaining);
                        }
                        Some(Either::Right(buf.into()))
                    }
                    None => self.default_as_span(song, ctx, tag_separator, strategy),
                }
            }
        }
    }
}

impl SizedPaneOrSplit {
    pub fn for_each_pane(
        &self,
        area: Rect,
        pane_callback: &mut impl FnMut(&ConfigPane, Rect, Block, Rect, Option<Color>) -> Result<()>,
        ctx: &Ctx,
    ) -> Result<()> {
        self.for_each_pane_custom_data(
            area,
            (),
            &mut |pane, pane_area, block, block_area, background_color, ()| {
                pane_callback(pane, pane_area, block, block_area, background_color)?;
                Ok(())
            },
            &mut |_, _, _, ()| Ok(()),
            ctx,
        )
    }

    pub fn for_each_pane_custom_data<T>(
        &self,
        area: Rect,
        mut custom_data: T,
        pane_callback: &mut impl FnMut(
            &ConfigPane,
            Rect,
            Block,
            Rect,
            Option<Color>,
            &mut T,
        ) -> Result<()>,
        split_callback: &mut impl FnMut(Block, Rect, Option<Color>, &mut T) -> Result<()>,
        ctx: &Ctx,
    ) -> Result<()> {
        let mut stack = vec![(self, area)];

        let song = ctx.find_current_song_in_queue().map(|(_, song)| song);
        while let Some((configured_panes, area)) = stack.pop() {
            match configured_panes {
                SizedPaneOrSplit::Pane(pane) => {
                    let mut block = Block::default()
                        .borders(pane.borders)
                        .border_set((&pane.border_symbols).into());
                    let bg_color = pane.background_color;
                    if pane.border_title.is_empty() {
                        let pane_area = block.inner(area);
                        pane_callback(pane, pane_area, block, area, bg_color, &mut custom_data)?;
                    } else {
                        let templs = PropertyTemplates::new(&pane.border_title);
                        let title = templs.format(song, ctx, &ctx.config);

                        block = block
                            .title(title)
                            .title_position(pane.border_title_position)
                            .title_alignment(pane.border_title_alignment);

                        let pane_area = block.inner(area);
                        pane_callback(pane, pane_area, block, area, bg_color, &mut custom_data)?;
                    }
                }
                SizedPaneOrSplit::Split {
                    direction,
                    panes,
                    background_color,
                    borders,
                    border_style,
                    border_title,
                    border_title_position,
                    border_title_alignment,
                    border_symbols,
                } => {
                    let parent_other_size = match direction {
                        ratatui::layout::Direction::Horizontal => area.height,
                        ratatui::layout::Direction::Vertical => area.width,
                    };
                    let constraints =
                        panes.iter().map(|pane| pane.size.into_constraint(parent_other_size));
                    let border_style = border_style.unwrap_or_else(|| ctx.config.as_border_style());
                    let mut block = Block::default()
                        .borders(*borders)
                        .border_style(border_style)
                        .border_set(border_symbols.into());

                    if border_title.is_empty() {
                        let pane_areas = block.inner(area);
                        let areas = Layout::new(*direction, constraints).split(pane_areas);
                        split_callback(block, area, *background_color, &mut custom_data)?;
                        stack.extend(
                            areas.iter().enumerate().map(|(idx, area)| (&panes[idx].pane, *area)),
                        );
                    } else {
                        let templs = PropertyTemplates::new(border_title);
                        let title = templs.format(song, ctx, &ctx.config);
                        block = block
                            .title(title)
                            .title_position(*border_title_position)
                            .title_alignment(*border_title_alignment);

                        let pane_areas = block.inner(area);
                        let areas = Layout::new(*direction, constraints).split(pane_areas);
                        split_callback(block, area, *background_color, &mut custom_data)?;
                        stack.extend(
                            areas.iter().enumerate().map(|(idx, area)| (&panes[idx].pane, *area)),
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod format_tests {
    use std::{collections::HashMap, time::Duration};

    use either::Either;
    use ratatui::{style::Style, text::Span};
    use rmpc_mpd::commands::{Song, State, Status, Volume, status::OnOffOneshot};
    use rstest::rstest;

    use crate::{
        config::theme::{
            StyleFile,
            TagResolutionStrategy,
            properties::{
                Property,
                PropertyKind,
                PropertyKindOrText,
                SongProperty,
                StatusProperty,
                StatusPropertyFile,
            },
        },
        ctx::Ctx,
        tests::fixtures::ctx,
        ui::song_ext::SongExt as _,
    };

    mod replace {
        use super::*;
        use crate::config::theme::{SymbolsConfig, properties::Transform};

        #[rstest]
        // simple 1:1 replace
        #[case(PropertyKindOrText::Text("abcdefgh".into()),
            None,
            "abcdefgh",
            PropertyKindOrText::Text("replaced text".into()),
            None,
            "replaced text")]
        // No replace input found
        #[case(PropertyKindOrText::Text("a".into()),
            None,
            "abcdefgh",
            PropertyKindOrText::Text("replaced text".into()),
            None,
            "a")]
        // Replace of group
        #[case(PropertyKindOrText::Group(vec![Property { kind: PropertyKindOrText::Text("a".into()), style: None, default: None }, Property { kind: PropertyKindOrText::Text("b".into()), style: None, default: None }]),
            None,
            "ab",
            PropertyKindOrText::Text("replaced text".into()),
            None,
            "replaced text")]
        // No replace of input found, fallback to original default
        #[case(PropertyKindOrText::Sticker("does not exist".into()),
            Some(PropertyKindOrText::Text("original default".into())),
            "does not match",
            PropertyKindOrText::Text("replaced text".into()),
            None,
            "original default")]
        // Replace found, but resolved to None - use replacement's default
        #[case(PropertyKindOrText::Text("a".into()),
            Some(PropertyKindOrText::Text("original default".into())),
            "a",
            PropertyKindOrText::Sticker("does not exist".into()),
            Some(PropertyKindOrText::Text("replacement default".into())),
            "replacement default")]
        fn as_span(
            #[case] input_props: PropertyKindOrText<PropertyKind>,
            #[case] input_default: Option<PropertyKindOrText<PropertyKind>>,
            #[case] input: String,
            #[case] replace_props: PropertyKindOrText<PropertyKind>,
            #[case] replace_default: Option<PropertyKindOrText<PropertyKind>>,
            #[case] expected: String,
            ctx: Ctx,
        ) {
            let format = Property::<PropertyKind> {
                kind: PropertyKindOrText::Transform(Transform::Replace {
                    content: Box::new(Property { kind: input_props, style: None, default: None }),
                    replacements: [(input, Property {
                        kind: replace_props,
                        style: None,
                        default: replace_default
                            .map(|d| Box::new(Property { kind: d, style: None, default: None })),
                    })]
                    .into_iter()
                    .collect(),
                }),
                style: None,
                default: input_default
                    .map(|d| Box::new(Property { kind: d, style: None, default: None })),
            };

            let result = format.as_span(None, &ctx, "", TagResolutionStrategy::All);

            assert_eq!(
                match result {
                    Some(Either::Left(v)) => Some(v.content.into_owned()),
                    Some(Either::Right(v)) =>
                        Some(v.iter().map(|s| s.content.clone()).collect::<String>()),
                    None => None,
                },
                Some(expected)
            );
        }

        #[rstest]
        // simple 1:1 replace
        #[case(PropertyKindOrText::Text("abcdefgh".into()),
            None,
            "abcdefgh",
            PropertyKindOrText::Text("replaced text".into()),
            None,
            "replaced text")]
        // No replace input found
        #[case(PropertyKindOrText::Text("a".into()),
            None,
            "abcdefgh",
            PropertyKindOrText::Text("replaced text".into()),
            None,
            "a")]
        // Replace of group
        #[case(PropertyKindOrText::Group(vec![Property { kind: PropertyKindOrText::Text("a".into()), style: None, default: None }, Property { kind: PropertyKindOrText::Text("b".into()), style: None, default: None }]),
            None,
            "ab",
            PropertyKindOrText::Text("replaced text".into()),
            None,
            "replaced text")]
        // No replace of input found, fallback to original default
        #[case(PropertyKindOrText::Sticker("does not exist".into()),
            Some(PropertyKindOrText::Text("original default".into())),
            "does not match",
            PropertyKindOrText::Text("replaced text".into()),
            None,
            "original default")]
        // Replace found, but resolved to None - use replacement's default
        #[case(PropertyKindOrText::Text("a".into()),
            Some(PropertyKindOrText::Text("original default".into())),
            "a",
            PropertyKindOrText::Sticker("does not exist".into()),
            Some(PropertyKindOrText::Text("replacement default".into())),
            "replacement default")]
        fn as_string(
            #[case] input_props: PropertyKindOrText<SongProperty>,
            #[case] input_default: Option<PropertyKindOrText<SongProperty>>,
            #[case] input: String,
            #[case] replace_props: PropertyKindOrText<SongProperty>,
            #[case] replace_default: Option<PropertyKindOrText<SongProperty>>,
            #[case] expected: &str,
            ctx: Ctx,
        ) {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Transform(Transform::Replace {
                    content: Box::new(Property { kind: input_props, style: None, default: None }),
                    replacements: [(input, Property {
                        kind: replace_props,
                        style: None,
                        default: replace_default
                            .map(|d| Box::new(Property { kind: d, style: None, default: None })),
                    })]
                    .into_iter()
                    .collect(),
                }),
                style: None,
                default: input_default
                    .map(|d| Box::new(Property { kind: d, style: None, default: None })),
            };

            let result = format.as_string(None, "", TagResolutionStrategy::All, &ctx);

            assert_eq!(result, Some(expected.to_string()));
        }

        #[rstest]
        #[rstest]
        // simple 1:1 replace
        #[case(PropertyKindOrText::Text("abcdefgh".into()),
            None,
            "abcdefgh",
            PropertyKindOrText::Text("replaced text".into()),
            None,
            "replaced text")]
        // No replace input found
        #[case(PropertyKindOrText::Text("a".into()),
            None,
            "abcdefgh",
            PropertyKindOrText::Text("replaced text".into()),
            None,
            "a")]
        // Replace of group
        #[case(PropertyKindOrText::Group(vec![Property { kind: PropertyKindOrText::Text("a".into()), style: None, default: None }, Property { kind: PropertyKindOrText::Text("b".into()), style: None, default: None }]),
            None,
            "ab",
            PropertyKindOrText::Text("replaced text".into()),
            None,
            "replaced text")]
        // No replace of input found, fallback to original default
        #[case(PropertyKindOrText::Sticker("does not exist".into()),
            Some(PropertyKindOrText::Text("original default".into())),
            "does not match",
            PropertyKindOrText::Text("replaced text".into()),
            None,
            "original default")]
        // Replace found, but resolved to None - use replacement's default
        #[case(PropertyKindOrText::Text("a".into()),
            Some(PropertyKindOrText::Text("original default".into())),
            "a",
            PropertyKindOrText::Sticker("does not exist".into()),
            Some(PropertyKindOrText::Text("replacement default".into())),
            "replacement default")]
        fn as_line_ellipsized(
            #[case] input_props: PropertyKindOrText<SongProperty>,
            #[case] input_default: Option<PropertyKindOrText<SongProperty>>,
            #[case] input: String,
            #[case] replace_props: PropertyKindOrText<SongProperty>,
            #[case] replace_default: Option<PropertyKindOrText<SongProperty>>,
            #[case] expected: String,
            ctx: Ctx,
        ) {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Transform(Transform::Replace {
                    content: Box::new(Property { kind: input_props, style: None, default: None }),
                    replacements: [(input, Property {
                        kind: replace_props,
                        style: None,
                        default: replace_default
                            .map(|d| Box::new(Property { kind: d, style: None, default: None })),
                    })]
                    .into_iter()
                    .collect(),
                }),
                style: None,
                default: input_default
                    .map(|d| Box::new(Property { kind: d, style: None, default: None })),
            };

            let song = Song::default();
            let result = song.as_line_ellipsized(
                &format,
                999,
                &SymbolsConfig::default(),
                "",
                TagResolutionStrategy::All,
                &ctx,
            );

            assert_eq!(
                result.map(|line| line.spans.iter().map(|s| s.content.clone()).collect::<String>()),
                Some(expected)
            );
        }
    }

    mod truncate {
        use itertools::Itertools;
        use ratatui::text::Line;

        use super::*;
        use crate::config::theme::{SymbolsConfig, properties::Transform};

        #[rstest]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 0, false, Either::Left(""))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 0, true, Either::Left(""))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 3, false, Either::Left("abc"))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 3, true, Either::Left("fgh"))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 8, false, Either::Left("abcdefgh"))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 8, true, Either::Left("abcdefgh"))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 99, false, Either::Left("abcdefgh"))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 99, true, Either::Left("abcdefgh"))]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 3, false, Either::Right(vec!["ab", "c"]))]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 3, true, Either::Right(vec!["f", "gh"]))]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 8, false, Either::Right(vec!["ab", "cd", "ef", "gh"]))]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 8, true, Either::Right(vec!["ab", "cd", "ef", "gh"]))]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 99, false, Either::Right(vec!["ab", "cd", "ef", "gh"]))]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 99, true, Either::Right(vec!["ab", "cd", "ef", "gh"]))]
        fn as_span(
            ctx: Ctx,
            #[case] props: PropertyKindOrText<PropertyKind>,
            #[case] length: usize,
            #[case] from_start: bool,
            #[case] expected: Either<&str, Vec<&str>>,
        ) {
            let format = Property::<PropertyKind> {
                kind: PropertyKindOrText::Transform(Transform::Truncate {
                    content: Box::new(Property { kind: props, style: None, default: None }),
                    length,
                    from_start,
                }),
                style: None,
                default: None,
            };

            let result = format.as_span(None, &ctx, "", TagResolutionStrategy::All);

            assert_eq!(
                result,
                Some(match expected {
                    Either::Left(value) =>
                        either::Either::<Span<'_>, Vec<Span<'_>>>::Left(Span::raw(value)),
                    Either::Right(values) => either::Either::<Span<'_>, Vec<Span<'_>>>::Right(
                        values.into_iter().map(Span::raw).collect()
                    ),
                })
            );
        }

        #[rstest]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 0, false, "")]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 0, true, "")]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 3, false, "abc")]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 3, true, "fgh")]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 8, false, "abcdefgh")]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 8, true, "abcdefgh")]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 99, false, "abcdefgh")]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 99, true, "abcdefgh")]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 3, false, "abc")]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 3, true, "fgh")]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 8, false, "abcdefgh")]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 8, true, "abcdefgh")]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 99, false, "abcdefgh")]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 99, true, "abcdefgh")]
        fn as_string(
            #[case] props: PropertyKindOrText<SongProperty>,
            #[case] length: usize,
            #[case] from_start: bool,
            #[case] expected: &str,
            ctx: Ctx,
        ) {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Transform(Transform::Truncate {
                    content: Box::new(Property { kind: props, style: None, default: None }),
                    length,
                    from_start,
                }),
                style: None,
                default: None,
            };

            let result = format.as_string(None, "", TagResolutionStrategy::All, &ctx);

            assert_eq!(result, Some(expected.to_string()));
        }

        #[rstest]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 0, false, Either::Left(""))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 0, true, Either::Left(""))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 3, false, Either::Left("abc"))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 3, true, Either::Left("fgh"))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 8, false, Either::Left("abcdefgh"))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 8, true, Either::Left("abcdefgh"))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 99, false, Either::Left("abcdefgh"))]
        #[case(PropertyKindOrText::Text("abcdefgh".into()), 99, true, Either::Left("abcdefgh"))]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 3, false, Either::Right(vec!["ab", "c"]))]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 3, true, Either::Right(vec!["f", "gh"]))]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 8, false, Either::Right(vec!["ab", "cd", "ef", "gh"]))]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 8, true, Either::Right(vec!["ab", "cd", "ef", "gh"]))]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 99, false, Either::Right(vec!["ab", "cd", "ef", "gh"]))]
        #[case(PropertyKindOrText::Group(vec![
                Property::builder().kind(PropertyKindOrText::Text("ab".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("cd".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("ef".into())).build(),
                Property::builder().kind(PropertyKindOrText::Text("gh".into())).build(),
            ]), 99, true, Either::Right(vec!["ab", "cd", "ef", "gh"]))]
        fn as_line_ellipsized(
            #[case] props: PropertyKindOrText<SongProperty>,
            #[case] length: usize,
            #[case] from_start: bool,
            #[case] expected: Either<&str, Vec<&str>>,
            ctx: Ctx,
        ) {
            let format = Property::<SongProperty> {
                kind: PropertyKindOrText::Transform(Transform::Truncate {
                    content: Box::new(Property { kind: props, style: None, default: None }),
                    length,
                    from_start,
                }),
                style: None,
                default: None,
            };

            let song = Song::default();
            let result = song.as_line_ellipsized(
                &format,
                999,
                &SymbolsConfig::default(),
                "",
                TagResolutionStrategy::All,
                &ctx,
            );

            assert_eq!(
                result,
                Some(match expected {
                    Either::Left(value) => Line::from(value),
                    Either::Right(values) =>
                        Line::from(values.into_iter().map(Span::raw).collect_vec()),
                })
            );
        }
    }

    mod correct_values {
        use super::*;

        #[rstest]
        #[case(SongProperty::Title, "title")]
        #[case(SongProperty::Artist, "artist")]
        #[case(SongProperty::Album, "album")]
        #[case(SongProperty::Track, "123")]
        #[case(SongProperty::Duration, "2:03")]
        #[case(SongProperty::Other("track".to_string()), "123")]
        fn song_property_resolves_correctly(
            #[case] prop: SongProperty,
            #[case] expected: &str,
            ctx: Ctx,
        ) {
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
                last_modified: chrono::Utc::now(),
                added: None,
            };

            let result = format.as_string(Some(&song), "", TagResolutionStrategy::All, &ctx);

            assert_eq!(result, Some(expected.to_string()));
        }

        #[rstest]
        #[case(StatusProperty::Volume, "100")]
        #[case(StatusProperty::Elapsed, "2:03")]
        #[case(StatusProperty::Duration, "2:03")]
        #[case(StatusProperty::Crossfade, "3")]
        #[case(StatusProperty::Bitrate, "123")]
        fn status_property_resolves_correctly(
            mut ctx: Ctx,
            #[case] prop: StatusProperty,
            #[case] expected: &str,
        ) {
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
                last_modified: chrono::Utc::now(),
                added: None,
            };
            ctx.status = Status {
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

            let result = format.as_span(Some(&song), &ctx, "", TagResolutionStrategy::All);

            assert_eq!(
                result,
                Some(either::Either::<Span<'_>, Vec<Span<'_>>>::Left(Span::raw(expected)))
            );
        }

        #[rstest]
        // Standard format tests (no separator = MM:SS format)
        #[case(StatusProperty::QueueTimeTotal { separator: None }, "6:09", Duration::from_secs(0))]
        #[case(StatusProperty::QueueTimeTotal { separator: Some(String::new())}, "6m9s", Duration::from_secs(0))]
        #[case(StatusProperty::QueueTimeRemaining { separator: None }, "6:09", Duration::from_secs(0))]
        #[case(StatusProperty::QueueTimeRemaining { separator: Some(String::new()) }, "6m9s", Duration::from_secs(0))]
        // With elapsed time, remaining should subtract elapsed from current song
        #[case(StatusProperty::QueueTimeRemaining { separator: None }, "5:49", Duration::from_secs(20))]
        #[case(StatusProperty::QueueTimeRemaining { separator: None }, "5:09", Duration::from_secs(60))]
        // Verbose format tests (with separator = verbose format)
        #[case(StatusProperty::QueueTimeTotal { separator: Some(",".to_string()) }, "6m,9s", Duration::from_secs(0))]
        #[case(StatusProperty::QueueTimeRemaining { separator: Some(",".to_string()) }, "6m,9s", Duration::from_secs(0))]
        #[case(StatusProperty::QueueTimeRemaining { separator: Some(",".to_string()) }, "5m,49s", Duration::from_secs(20))]
        fn queue_time_property_resolves_correctly(
            mut ctx: Ctx,
            #[case] prop: StatusProperty,
            #[case] expected: &str,
            #[case] elapsed: Duration,
        ) {
            let format = Property::<PropertyKind> {
                kind: PropertyKindOrText::Property(PropertyKind::Status(prop)),
                style: None,
                default: None,
            };

            // Test with a fake current song
            let current_song = Song {
                id: 0,
                file: "current.mp3".to_owned(),
                duration: Some(Duration::from_secs(123)),
                metadata: HashMap::from([
                    ("title".to_string(), "Current Song".into()),
                    ("artist".to_string(), "Artist".into()),
                ]),
                last_modified: chrono::Utc::now(),
                added: None,
            };

            // Set up the app context with a fake queue and status
            let mut queue = vec![current_song.clone()];
            queue.push(Song {
                id: 1,
                file: "song1.mp3".to_owned(),
                duration: Some(Duration::from_secs(123)),
                metadata: HashMap::from([("title".to_string(), "Song 1".into())]),
                last_modified: chrono::Utc::now(),
                added: None,
            });
            queue.push(Song {
                id: 2,
                file: "song2.mp3".to_owned(),
                duration: Some(Duration::from_secs(123)),
                metadata: HashMap::from([("title".to_string(), "Song 2".into())]),
                last_modified: chrono::Utc::now(),
                added: None,
            });

            ctx.queue = queue;
            ctx.status = Status {
                elapsed,
                duration: Duration::from_secs(123),
                state: State::Play,
                song: Some(0),
                songid: Some(0),
                ..Default::default()
            };
            ctx.cached_queue_time_total =
                ctx.queue.iter().map(|s| s.duration.unwrap_or_default()).sum();

            let result = format.as_span(Some(&current_song), &ctx, "", TagResolutionStrategy::All);

            assert_eq!(
                result,
                Some(either::Either::<Span<'_>, Vec<Span<'_>>>::Left(Span::raw(expected)))
            );
        }

        #[rstest]
        // no current song or if the queue is empty, the queue time should be 0:00
        #[case(StatusProperty::QueueTimeTotal { separator: None }, "0:00")]
        #[case(StatusProperty::QueueTimeRemaining { separator: None }, "0:00")]
        #[case(StatusProperty::QueueTimeTotal { separator: Some(",".to_string()) }, "0s")]
        #[case(StatusProperty::QueueTimeRemaining { separator: Some(",".to_string()) }, "0s")]
        fn queue_time_property_no_current_song(
            mut ctx: Ctx,
            #[case] prop: StatusProperty,
            #[case] expected: &str,
        ) {
            let format = Property::<PropertyKind> {
                kind: PropertyKindOrText::Property(PropertyKind::Status(prop)),
                style: None,
                default: None,
            };

            ctx.queue = vec![];
            ctx.status = Status { state: State::Stop, ..Default::default() };

            let result = format.as_span(None, &ctx, "", TagResolutionStrategy::All);

            assert_eq!(
                result,
                Some(either::Either::<Span<'_>, Vec<Span<'_>>>::Left(Span::raw(expected)))
            );
        }

        #[rstest]
        // Test edge case: songs without duration
        // if somehow the queue contains songs without duration, the queue time should still be 0:00
        #[case(StatusProperty::QueueTimeTotal { separator: None }, "0:00")]
        #[case(StatusProperty::QueueTimeRemaining { separator: None }, "0:00")]
        fn queue_time_property_no_duration(
            mut ctx: Ctx,
            #[case] prop: StatusProperty,
            #[case] expected: &str,
        ) {
            let format = Property::<PropertyKind> {
                kind: PropertyKindOrText::Property(PropertyKind::Status(prop)),
                style: None,
                default: None,
            };

            let song_no_duration = Song {
                id: 0,
                file: "no_duration.mp3".to_owned(),
                duration: None,
                metadata: HashMap::from([("title".to_string(), "No Duration".into())]),
                last_modified: chrono::Utc::now(),
                added: None,
            };

            ctx.queue = vec![song_no_duration.clone()];
            ctx.status = Status { state: State::Play, song: Some(0), ..Default::default() };

            let result =
                format.as_span(Some(&song_no_duration), &ctx, "", TagResolutionStrategy::All);

            assert_eq!(
                result,
                Some(either::Either::<Span<'_>, Vec<Span<'_>>>::Left(Span::raw(expected)))
            );
        }

        #[rstest]
        #[case("otherplay", "otherstopped", "otherpaused", State::Play, "otherplay")]
        #[case("otherplay", "otherstopped", "otherpaused", State::Pause, "otherpaused")]
        #[case("otherplay", "otherstopped", "otherpaused", State::Stop, "otherstopped")]
        fn playback_state_label_is_correct(
            mut ctx: Ctx,
            #[case] playing_label: &'static str,
            #[case] stopped_label: &'static str,
            #[case] paused_label: &'static str,
            #[case] state: State,
            #[case] expected_label: &str,
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
            ctx.status = Status { state, ..Default::default() };

            let result = format.as_span(Some(&song), &ctx, "", TagResolutionStrategy::All);

            assert_eq!(
                result,
                Some(either::Either::<Span<'_>, Vec<Span<'_>>>::Left(Span::raw(expected_label)))
            );
        }

        #[rstest]
        #[case(StatusPropertyFile::ConsumeV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), oneshot_label: "1111".to_string(), on_style: None, off_style: None, oneshot_style: None }, Status { consume: OnOffOneshot::On, ..Default::default() }, "ye")]
        #[case(StatusPropertyFile::ConsumeV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), oneshot_label: "1111".to_string(), on_style: None, off_style: None, oneshot_style: None }, Status { consume: OnOffOneshot::Off, ..Default::default() }, "naw")]
        #[case(StatusPropertyFile::ConsumeV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), oneshot_label: "1111".to_string(), on_style: None, off_style: None, oneshot_style: None }, Status { consume: OnOffOneshot::Oneshot, ..Default::default() }, "1111")]
        #[case(StatusPropertyFile::SingleV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), oneshot_label: "1111".to_string(), on_style: None, off_style: None, oneshot_style: None }, Status { single: OnOffOneshot::On, ..Default::default() }, "ye")]
        #[case(StatusPropertyFile::SingleV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), oneshot_label: "1111".to_string(), on_style: None, off_style: None, oneshot_style: None }, Status { single: OnOffOneshot::Off, ..Default::default() }, "naw")]
        #[case(StatusPropertyFile::SingleV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), oneshot_label: "1111".to_string(), on_style: None, off_style: None, oneshot_style: None }, Status { single: OnOffOneshot::Oneshot, ..Default::default() }, "1111")]
        #[case(StatusPropertyFile::RandomV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), on_style: None, off_style: None }, Status { random: true, ..Default::default() }, "ye")]
        #[case(StatusPropertyFile::RandomV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), on_style: None, off_style: None }, Status { random: false, ..Default::default() }, "naw")]
        #[case(StatusPropertyFile::RepeatV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), on_style: None, off_style: None }, Status { repeat: true, ..Default::default() }, "ye")]
        #[case(StatusPropertyFile::RepeatV2 { on_label: "ye".to_string(), off_label: "naw".to_string(), on_style: None, off_style: None }, Status { repeat: false, ..Default::default() }, "naw")]
        #[case(StatusPropertyFile::Consume, Status { consume: OnOffOneshot::On, ..Default::default() }, "On")]
        #[case(StatusPropertyFile::Consume, Status { consume: OnOffOneshot::Off, ..Default::default() }, "Off")]
        #[case(StatusPropertyFile::Consume, Status { consume: OnOffOneshot::Oneshot, ..Default::default() }, "OS")]
        #[case(StatusPropertyFile::Repeat, Status { repeat: true, ..Default::default() }, "On")]
        #[case(StatusPropertyFile::Repeat, Status { repeat: false, ..Default::default() }, "Off")]
        #[case(StatusPropertyFile::Random, Status { random: true, ..Default::default() }, "On")]
        #[case(StatusPropertyFile::Random, Status { random: false, ..Default::default() }, "Off")]
        #[case(StatusPropertyFile::Single, Status { single: OnOffOneshot::On, ..Default::default() }, "On")]
        #[case(StatusPropertyFile::Single, Status { single: OnOffOneshot::Off, ..Default::default() }, "Off")]
        #[case(StatusPropertyFile::Single, Status { single: OnOffOneshot::Oneshot, ..Default::default() }, "OS")]
        fn on_off_states_label_is_correct(
            mut ctx: Ctx,
            #[case] prop: StatusPropertyFile,
            #[case] status: Status,
            #[case] expected_label: &str,
        ) {
            let format = Property::<PropertyKind> {
                kind: PropertyKindOrText::Property(PropertyKind::Status(prop.try_into().unwrap())),
                style: None,
                default: None,
            };

            let song = Song { id: 1, file: "file".to_owned(), ..Default::default() };

            ctx.status = status;

            let result = format.as_span(Some(&song), &ctx, "", TagResolutionStrategy::All);

            assert_eq!(result, Some(Either::Left(Span::raw(expected_label))));
        }

        #[rstest]
        #[case(StatusPropertyFile::ConsumeV2 { on_style: Some(StyleFile::builder().fg("red".to_string()).build()), off_style: Some(StyleFile::builder().fg("green".to_string()).build()), oneshot_style: Some(StyleFile::builder().fg("blue".to_string()).build()), on_label: String::new(), off_label: String::new(), oneshot_label: String::new() }, Status { consume: OnOffOneshot::On, ..Default::default() }, Some(Style::default().red()))]
        #[case(StatusPropertyFile::SingleV2  { on_style: Some(StyleFile::builder().fg("red".to_string()).build()), off_style: Some(StyleFile::builder().fg("green".to_string()).build()), oneshot_style: Some(StyleFile::builder().fg("blue".to_string()).build()),  on_label: String::new(), off_label: String::new(), oneshot_label: String::new() }, Status { single: OnOffOneshot::On, ..Default::default() }, Some(Style::default().red()))]
        #[case(StatusPropertyFile::RandomV2  { on_style: Some(StyleFile::builder().fg("red".to_string()).build()), off_style: Some(StyleFile::builder().fg("green".to_string()).build()), on_label: String::new(), off_label: String::new() }, Status { random: true, ..Default::default() }, Some(Style::default().red()))]
        #[case(StatusPropertyFile::RepeatV2  { on_style: Some(StyleFile::builder().fg("red".to_string()).build()), off_style: Some(StyleFile::builder().fg("green".to_string()).build()), on_label: String::new(), off_label: String::new() }, Status { repeat: true, ..Default::default() }, Some(Style::default().red()))]
        #[case(StatusPropertyFile::ConsumeV2 { on_style: None, off_style: None, oneshot_style: None, on_label: String::new(), off_label: String::new(), oneshot_label: String::new() }, Status { consume: OnOffOneshot::On, ..Default::default() }, None)]
        #[case(StatusPropertyFile::SingleV2  { on_style: None, off_style: None, oneshot_style: None, on_label: String::new(), off_label: String::new(), oneshot_label: String::new() }, Status { single: OnOffOneshot::On, ..Default::default() }, None)]
        #[case(StatusPropertyFile::RandomV2  { on_style: None, off_style: None, on_label: String::new(), off_label: String::new() }, Status { random: true, ..Default::default() }, None)]
        #[case(StatusPropertyFile::RepeatV2  { on_style: None, off_style: None, on_label: String::new(), off_label: String::new() }, Status { repeat: true, ..Default::default() }, None)]
        fn on_off_oneshot_styles_are_correct(
            mut ctx: Ctx,
            #[case] prop: StatusPropertyFile,
            #[case] status: Status,
            #[case] expected_style: Option<Style>,
        ) {
            let format = Property::<PropertyKind> {
                kind: PropertyKindOrText::Property(PropertyKind::Status(prop.try_into().unwrap())),
                style: None,
                default: None,
            };

            let song = Song { id: 1, file: "file".to_owned(), ..Default::default() };

            ctx.status = status;

            let result = format.as_span(Some(&song), &ctx, "", TagResolutionStrategy::All);

            dbg!(&result);
            assert_eq!(
                result,
                Some(Either::Left(Span::styled(String::new(), expected_style.unwrap_or_default())))
            );
        }
    }

    mod property {
        use super::*;

        #[rstest]
        fn works(ctx: Ctx) {
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

            let result = format.as_string(Some(&song), "", TagResolutionStrategy::All, &ctx);

            assert_eq!(result, Some("title".to_owned()));
        }

        #[rstest]
        fn falls_back(ctx: Ctx) {
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

            let result = format.as_string(Some(&song), "", TagResolutionStrategy::All, &ctx);

            assert_eq!(result, Some("fallback".to_owned()));
        }

        #[rstest]
        fn falls_back_to_none(ctx: Ctx) {
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

            let result = format.as_string(Some(&song), "", TagResolutionStrategy::All, &ctx);

            assert_eq!(result, None);
        }
    }

    mod text {
        use super::*;

        #[rstest]
        fn works(ctx: Ctx) {
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

            let result = format.as_string(Some(&song), "", TagResolutionStrategy::All, &ctx);

            assert_eq!(result, Some("test".to_owned()));
        }

        #[rstest]
        fn fallback_is_ignored(ctx: Ctx) {
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

            let result = format.as_string(Some(&song), "", TagResolutionStrategy::All, &ctx);

            assert_eq!(result, Some("test".to_owned()));
        }
    }

    mod group {
        use super::*;

        #[rstest]
        fn group_no_fallback(ctx: Ctx) {
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

            let result = format.as_string(Some(&song), "", TagResolutionStrategy::All, &ctx);

            assert_eq!(result, None);
        }

        #[rstest]
        fn group_fallback(ctx: Ctx) {
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

            let result = format.as_string(Some(&song), "", TagResolutionStrategy::All, &ctx);

            assert_eq!(result, Some("fallback".to_owned()));
        }

        #[rstest]
        fn group_resolved(ctx: Ctx) {
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

            let result = format.as_string(Some(&song), "", TagResolutionStrategy::All, &ctx);

            assert_eq!(result, Some("titletext".to_owned()));
        }

        #[rstest]
        fn group_fallback_in_group(ctx: Ctx) {
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

            let result = format.as_string(Some(&song), "", TagResolutionStrategy::All, &ctx);

            assert_eq!(result, Some("fallbacktext".to_owned()));
        }

        #[rstest]
        fn group_nesting(ctx: Ctx) {
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

            let result = format.as_string(Some(&song), "", TagResolutionStrategy::All, &ctx);

            assert_eq!(result, Some("innerfallbackouter".to_owned()));
        }
    }
}
