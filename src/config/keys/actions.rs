use std::{borrow::Cow, fmt::Write, ops::Range, sync::Arc};

use anyhow::bail;
use itertools::Itertools;
use strum::{Display, EnumDiscriminants, VariantArray};

use super::ToDescription;
use crate::{
    config::{tabs::TabName, utils::tilde_expand},
    mpd::{QueuePosition, commands::Song},
    shared::macros::status_warn,
};

// Global actions

#[derive(Debug, Display, Clone, EnumDiscriminants, PartialEq, Eq)]
#[strum_discriminants(derive(VariantArray))]
pub enum GlobalAction {
    Quit,
    ShowHelp,
    ShowCurrentSongInfo,
    ShowOutputs,
    ShowDecoders,
    #[strum(to_string = "Partition({name:?})")]
    Partition {
        name: Option<String>,
        autocreate: bool,
    },
    AddRandom,
    NextTrack,
    PreviousTrack,
    Stop,
    ToggleRepeat,
    ToggleSingle,
    ToggleRandom,
    ToggleConsume,
    ToggleSingleOnOff,
    ToggleConsumeOnOff,
    TogglePause,
    VolumeUp,
    VolumeDown,
    CrossfadeUp,
    CrossfadeDown,
    SeekForward,
    SeekBack,
    SeekToStart,
    Update,
    Rescan,
    CommandMode,
    NextTab,
    PreviousTab,
    #[strum(to_string = "SwitchToTab({0})")]
    SwitchToTab(TabName),
    Command {
        command: String,
        description: Option<String>,
    },
    ExternalCommand {
        command: Arc<Vec<String>>,
        description: Option<String>,
    },
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum GlobalActionFile {
    Quit,
    ShowHelp,
    ShowCurrentSongInfo,
    ShowOutputs,
    ShowDecoders,
    Partition {
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        autocreate: bool,
    },
    NextTrack,
    PreviousTrack,
    Stop,
    ToggleRepeat,
    ToggleSingle,
    ToggleRandom,
    ToggleConsume,
    ToggleSingleOnOff,
    ToggleConsumeOnOff,
    TogglePause,
    VolumeUp,
    VolumeDown,
    CrossfadeUp,
    CrossfadeDown,
    SeekForward,
    SeekBack,
    SeekToStart,
    Update,
    Rescan,
    NextTab,
    PreviousTab,
    SwitchToTab(String),
    QueueTab,
    DirectoriesTab,
    ArtistsTab,
    AlbumsTab,
    PlaylistsTab,
    SearchTab,
    CommandMode,
    Command {
        command: String,
        description: Option<String>,
    },
    ExternalCommand {
        command: Vec<String>,
        description: Option<String>,
    },
    AddRandom,
}

impl From<GlobalActionFile> for GlobalAction {
    fn from(value: GlobalActionFile) -> Self {
        match value {
            GlobalActionFile::Quit => GlobalAction::Quit,
            GlobalActionFile::ShowOutputs => GlobalAction::ShowOutputs,
            GlobalActionFile::ShowDecoders => GlobalAction::ShowDecoders,
            GlobalActionFile::ShowCurrentSongInfo => GlobalAction::ShowCurrentSongInfo,
            GlobalActionFile::CommandMode => GlobalAction::CommandMode,
            GlobalActionFile::Command { command, description } => {
                GlobalAction::Command { command, description }
            }
            GlobalActionFile::ShowHelp => GlobalAction::ShowHelp,
            GlobalActionFile::NextTrack => GlobalAction::NextTrack,
            GlobalActionFile::PreviousTrack => GlobalAction::PreviousTrack,
            GlobalActionFile::Stop => GlobalAction::Stop,
            GlobalActionFile::ToggleRepeat => GlobalAction::ToggleRepeat,
            GlobalActionFile::ToggleRandom => GlobalAction::ToggleRandom,
            GlobalActionFile::ToggleSingle => GlobalAction::ToggleSingle,
            GlobalActionFile::TogglePause => GlobalAction::TogglePause,
            GlobalActionFile::SeekForward => GlobalAction::SeekForward,
            GlobalActionFile::SeekBack => GlobalAction::SeekBack,
            GlobalActionFile::SeekToStart => GlobalAction::SeekToStart,
            GlobalActionFile::Update => GlobalAction::Update,
            GlobalActionFile::Rescan => GlobalAction::Rescan,
            GlobalActionFile::VolumeDown => GlobalAction::VolumeDown,
            GlobalActionFile::VolumeUp => GlobalAction::VolumeUp,
            GlobalActionFile::CrossfadeDown => GlobalAction::CrossfadeDown,
            GlobalActionFile::CrossfadeUp => GlobalAction::CrossfadeUp,
            GlobalActionFile::PreviousTab => GlobalAction::PreviousTab,
            GlobalActionFile::NextTab => GlobalAction::NextTab,
            GlobalActionFile::ToggleConsume => GlobalAction::ToggleConsume,
            GlobalActionFile::SwitchToTab(name) => GlobalAction::SwitchToTab(name.into()),
            GlobalActionFile::QueueTab => GlobalAction::SwitchToTab("Queue".into()),
            GlobalActionFile::DirectoriesTab => GlobalAction::SwitchToTab("Directories".into()),
            GlobalActionFile::ArtistsTab => GlobalAction::SwitchToTab("Artists".into()),
            GlobalActionFile::AlbumsTab => GlobalAction::SwitchToTab("Albums".into()),
            GlobalActionFile::PlaylistsTab => GlobalAction::SwitchToTab("Playlists".into()),
            GlobalActionFile::SearchTab => GlobalAction::SwitchToTab("Search".into()),
            GlobalActionFile::ExternalCommand { command, description } => {
                GlobalAction::ExternalCommand {
                    command: Arc::new(
                        command.into_iter().map(|v| tilde_expand(&v).into_owned()).collect_vec(),
                    ),
                    description,
                }
            }
            GlobalActionFile::AddRandom => GlobalAction::AddRandom,
            GlobalActionFile::ToggleSingleOnOff => GlobalAction::ToggleSingleOnOff,
            GlobalActionFile::ToggleConsumeOnOff => GlobalAction::ToggleConsumeOnOff,
            GlobalActionFile::Partition { name, autocreate } => {
                GlobalAction::Partition { name, autocreate }
            }
        }
    }
}

impl ToDescription for GlobalAction {
    fn to_description(&self) -> Cow<'static, str> {
        match self {
            GlobalAction::Quit => "Exit rmpc".into(),
            GlobalAction::ShowOutputs => "Show MPD outputs config".into(),
            GlobalAction::ShowDecoders => "Show MPD decoder plugins".into(),
            GlobalAction::ShowCurrentSongInfo => {
                "Show metadata of the currently playing song in a modal popup".into()
            }
            GlobalAction::ToggleRepeat => "Toggle repeat".into(),
            GlobalAction::ToggleSingle => {
                "Whether to stop playing after single track or repeat track/playlist when repeat is on".into()
            }
            GlobalAction::ToggleRandom => "Toggles random playback".into(),
            GlobalAction::ToggleConsume => "Remove song from the queue after playing".into(),
            GlobalAction::TogglePause => "Pause/Unpause playback".into(),
            GlobalAction::Stop => "Stop playback".into(),
            GlobalAction::VolumeUp => "Raise volume".into(),
            GlobalAction::VolumeDown => "Lower volume".into(),
            GlobalAction::CrossfadeUp => "Increase crossfade duration".into(),
            GlobalAction::CrossfadeDown => "Decrease crossfade duration".into(),
            GlobalAction::NextTrack => "Play next track in the queue".into(),
            GlobalAction::PreviousTrack => "Play previous track in the queue".into(),
            GlobalAction::SeekForward => "Seek currently playing track forwards".into(),
            GlobalAction::SeekBack => "Seek currently playing track backwards".into(),
            GlobalAction::SeekToStart => "Seek to the beginning of the currently playing track".into(),
            GlobalAction::Update => "Update music library".into(),
            GlobalAction::Rescan => "Rescan music library (incl. unmodified files)".into(),
            GlobalAction::NextTab => "Switch to next tab".into(),
            GlobalAction::PreviousTab => "Switch to previous tab".into(),
            GlobalAction::SwitchToTab(name) => Cow::Owned(format!("Switch directly to {name} tab")),
            GlobalAction::ShowHelp => "Show keybinds".into(),
            GlobalAction::CommandMode => "Enter command mode".into(),
            GlobalAction::Command { description: None, .. } => "Execute a command".into(),
            GlobalAction::Command { description: Some(desc), .. } => Cow::Owned(desc.to_owned()),
            GlobalAction::ExternalCommand { description: None, .. } => {
                "Execute an external command".into()
            }
            GlobalAction::ExternalCommand { description: Some(desc), .. } => Cow::Owned(desc.clone()),
            GlobalAction::AddRandom => "Add random songs to the queue".into(),
            GlobalAction::ToggleSingleOnOff => "Toggle single mode on or off, skipping oneshot".into(),
            GlobalAction::ToggleConsumeOnOff => "Toggle consume mode on or off, skipping oneshot".into(),
            GlobalAction::Partition { name: Some(name), .. }=> format!("Switch to '{name}' partition").into(),
            GlobalAction::Partition { name: None, .. }=> "Open partition management modal".into(),
        }
    }
}

// Albums actions

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum AlbumsActionsFile {}

#[derive(Debug, Display, Clone, Copy, PartialEq)]
pub enum AlbumsActions {}

impl From<AlbumsActionsFile> for AlbumsActions {
    fn from(_value: AlbumsActionsFile) -> Self {
        unreachable!()
    }
}

impl ToDescription for AlbumsActions {
    fn to_description(&self) -> Cow<'static, str> {
        "".into()
    }
}

// Artists actions

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum ArtistsActionsFile {}

#[derive(Debug, Display, Clone, Copy, PartialEq)]
pub enum ArtistsActions {}

impl ToDescription for ArtistsActions {
    fn to_description(&self) -> Cow<'static, str> {
        "".into()
    }
}

impl From<ArtistsActionsFile> for ArtistsActions {
    fn from(_value: ArtistsActionsFile) -> Self {
        unreachable!()
    }
}

// Directories actions

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum DirectoriesActionsFile {}

#[derive(Debug, Display, Clone, Copy, PartialEq)]
pub enum DirectoriesActions {}

impl ToDescription for DirectoriesActions {
    fn to_description(&self) -> Cow<'static, str> {
        "".into()
    }
}

impl From<DirectoriesActionsFile> for DirectoriesActions {
    fn from(_value: DirectoriesActionsFile) -> Self {
        unreachable!()
    }
}

// Logs actions
#[cfg(debug_assertions)]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum LogsActionsFile {
    Clear,
    ToggleScroll,
}

#[cfg(debug_assertions)]
#[allow(dead_code)]
#[derive(Debug, Display, Clone, Copy, PartialEq, Eq)]
pub enum LogsActions {
    Clear,
    ToggleScroll,
}

#[cfg(debug_assertions)]
impl From<LogsActionsFile> for LogsActions {
    fn from(value: LogsActionsFile) -> Self {
        match value {
            LogsActionsFile::Clear => LogsActions::Clear,
            LogsActionsFile::ToggleScroll => LogsActions::ToggleScroll,
        }
    }
}

#[cfg(debug_assertions)]
impl ToDescription for LogsActions {
    fn to_description(&self) -> Cow<'static, str> {
        match self {
            LogsActions::Clear => "Clear logs",
            LogsActions::ToggleScroll => "Toggle automatic scrolling when log gets added",
        }
        .into()
    }
}

// Queue actions

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub enum QueueActionsFile {
    Delete,
    DeleteAll,
    Play,
    #[deprecated]
    Save,
    AddToPlaylist,
    ShowInfo,
    JumpToCurrent,
    Shuffle,
}

#[derive(Debug, Display, Clone, Copy, EnumDiscriminants, PartialEq, Eq)]
#[strum_discriminants(derive(VariantArray))]
pub enum QueueActions {
    Delete,
    DeleteAll,
    Play,
    #[deprecated]
    Save,
    AddToPlaylist,
    JumpToCurrent,
    Shuffle,
    Unused,
}

impl From<QueueActionsFile> for QueueActions {
    fn from(value: QueueActionsFile) -> Self {
        match value {
            QueueActionsFile::Delete => QueueActions::Delete,
            QueueActionsFile::DeleteAll => QueueActions::DeleteAll,
            QueueActionsFile::Play => QueueActions::Play,
            QueueActionsFile::Save => QueueActions::Save,
            QueueActionsFile::AddToPlaylist => QueueActions::AddToPlaylist,
            QueueActionsFile::ShowInfo => QueueActions::Unused,
            QueueActionsFile::JumpToCurrent => QueueActions::JumpToCurrent,
            QueueActionsFile::Shuffle => QueueActions::Shuffle,
        }
    }
}

impl ToDescription for QueueActions {
    fn to_description(&self) -> Cow<'static, str> {
        match self {
            QueueActions::Delete => "Remove song under curor from the queue",
            QueueActions::DeleteAll => "Clear current queue",
            QueueActions::Play => "Play song under cursor",
            QueueActions::Save => "Save current queue as a new playlist",
            QueueActions::AddToPlaylist => "Add song under cursor to an existing playlist",
            QueueActions::Unused => "unused",
            QueueActions::JumpToCurrent => {
                "Moves the cursor in Queue table to the currently playing song"
            }
            QueueActions::Shuffle => "Shuffles the current queue",
        }
        .into()
    }
}

#[derive(
    Debug,
    Display,
    Default,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Ord,
    PartialOrd,
)]
pub enum Position {
    AfterCurrentSong,
    BeforeCurrentSong,
    AfterCurrentAlbum,
    BeforeCurrentAlbum,
    StartOfQueue,
    #[default]
    EndOfQueue,
    Replace,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
pub enum AddKind {
    Modal(Vec<(String, AddOpts)>),
    Action(AddOpts),
}

impl std::fmt::Display for AddKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddKind::Modal(modal) => write!(f, "Modal with options {:?} options", modal.len()),
            AddKind::Action(opts) => write!(
                f,
                "position: {}, autoplay: {}, all: {}",
                opts.position, opts.autoplay, opts.all
            ),
        }
    }
}

impl Default for AddKind {
    fn default() -> Self {
        AddKind::Modal(vec![
            ("At the end of queue".into(), AddOpts {
                autoplay: AutoplayKind::None,
                position: Position::EndOfQueue,
                all: false,
            }),
            ("At the start of queue".into(), AddOpts {
                autoplay: AutoplayKind::None,
                position: Position::StartOfQueue,
                all: false,
            }),
            ("After the current song".into(), AddOpts {
                autoplay: AutoplayKind::None,
                position: Position::AfterCurrentSong,
                all: false,
            }),
            ("Replace the queue".into(), AddOpts {
                autoplay: AutoplayKind::None,
                position: Position::Replace,
                all: false,
            }),
            ("Replace the queue and play".into(), AddOpts {
                autoplay: AutoplayKind::First,
                position: Position::Replace,
                all: false,
            }),
        ])
    }
}

#[derive(
    Debug,
    Default,
    Display,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Ord,
    PartialOrd,
)]
pub enum AutoplayKind {
    First,
    #[default]
    Hovered,
    HoveredOrFirst,
    None,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
pub struct AddOpts {
    #[serde(default)]
    pub autoplay: AutoplayKind,
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub position: Position,
}

impl AddOpts {
    pub fn autoplay_idx_and_queue_position(
        self,
        queue: &[Song],
        current_song_idx: Option<usize>,
        hovered_song_idx: Option<usize>,
    ) -> anyhow::Result<(Option<usize>, Option<QueuePosition>)> {
        let ranges = Self::to_album_ranges(queue);
        Ok((
            self.autoplay_idx(queue, current_song_idx, hovered_song_idx, &ranges)?,
            self.queue_position(current_song_idx, &ranges)?,
        ))
    }

    fn to_album_ranges(queue: &[Song]) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < queue.len() {
            let a = queue[i].metadata.get("album");
            let aa = queue[i].metadata.get("album_artist");
            let mut j = i + 1;
            while j < queue.len()
                && queue[j].metadata.get("album") == a
                && queue[j].metadata.get("album_artist") == aa
            {
                j += 1;
            }
            out.push(i..j);
            i = j;
        }
        out
    }

    fn queue_position(
        self,
        current_song_idx: Option<usize>,
        same_album_ranges: &[Range<usize>],
    ) -> anyhow::Result<Option<QueuePosition>> {
        macro_rules! find_range_or_bail {
            ($curr_idx:expr) => {
                same_album_ranges
                    .iter()
                    .find(|range| range.contains(&$curr_idx))
                    .ok_or_else(|| anyhow::anyhow!("Current song's album range not found"))?
            };
        }
        Ok(match self.position {
            Position::AfterCurrentSong => Some(QueuePosition::RelativeAdd(0)),
            Position::BeforeCurrentSong => Some(QueuePosition::RelativeSub(0)),
            Position::AfterCurrentAlbum => {
                let Some(current_song_idx) = current_song_idx else {
                    bail!("No current song to queue after its album");
                };

                let range = find_range_or_bail!(current_song_idx);

                Some(QueuePosition::Absolute(range.end))
            }
            Position::BeforeCurrentAlbum => {
                let Some(current_song_idx) = current_song_idx else {
                    bail!("No current song to queue before its album");
                };

                let range = find_range_or_bail!(current_song_idx);

                Some(QueuePosition::Absolute(range.start))
            }
            Position::StartOfQueue => Some(QueuePosition::Absolute(0)),
            Position::EndOfQueue => None,
            Position::Replace => None,
        })
    }

    fn autoplay_idx(
        self,
        queue: &[Song],
        current_song_idx: Option<usize>,
        hovered_song_idx: Option<usize>,
        same_album_ranges: &[Range<usize>],
    ) -> anyhow::Result<Option<usize>> {
        let queue_len = queue.len();
        macro_rules! find_range_or_bail {
            ($curr_idx:expr) => {
                same_album_ranges
                    .iter()
                    .find(|range| range.contains(&$curr_idx))
                    .ok_or_else(|| anyhow::anyhow!("Current song's album range not found"))?
            };
        }
        Ok(match (self.autoplay, current_song_idx, hovered_song_idx) {
            (AutoplayKind::First, Some(curr), _) => match self.position {
                Position::AfterCurrentSong => Some(curr + 1),
                Position::BeforeCurrentSong => Some(curr),
                Position::AfterCurrentAlbum => same_album_ranges
                    .iter()
                    .find(|range| range.contains(&curr))
                    .map(|album_range| album_range.end)
                    .or_else(|| {
                        status_warn!("Current song's album range not found");
                        None
                    }),
                Position::BeforeCurrentAlbum => same_album_ranges
                    .iter()
                    .find(|range| range.contains(&curr))
                    .map(|album_range| album_range.start)
                    .or_else(|| {
                        status_warn!("Current song's album range not found");
                        None
                    }),
                Position::StartOfQueue => Some(0),
                Position::EndOfQueue => Some(queue_len),
                Position::Replace => Some(0),
            },
            (AutoplayKind::First, None, _) => match self.position {
                Position::AfterCurrentSong => {
                    bail!("No current song to queue after");
                }
                Position::BeforeCurrentSong => {
                    bail!("No current song to queue before");
                }
                Position::AfterCurrentAlbum => {
                    bail!("No current song to queue after its album");
                }
                Position::BeforeCurrentAlbum => {
                    bail!("No current song to queue before its album");
                }
                Position::StartOfQueue => Some(0),
                Position::EndOfQueue => Some(queue_len),
                Position::Replace => Some(0),
            },
            (AutoplayKind::Hovered, curr, hovered) => match self.position {
                Position::AfterCurrentSong => {
                    let Some(current_song_idx) = curr else {
                        bail!("No current song to queue after");
                    };

                    hovered.map(|i| i + 1 + current_song_idx)
                }
                Position::BeforeCurrentSong => {
                    let Some(current_song_idx) = curr else {
                        bail!("No current song to queue before");
                    };

                    hovered.map(|i| i + current_song_idx)
                }
                Position::AfterCurrentAlbum => {
                    let Some(current_song_idx) = curr else {
                        bail!("No current song to queue after its album");
                    };

                    let range = find_range_or_bail!(current_song_idx);
                    hovered.map(|i| i + range.end)
                }
                Position::BeforeCurrentAlbum => {
                    let Some(current_song_idx) = curr else {
                        bail!("No current song to queue before its album");
                    };

                    let range = find_range_or_bail!(current_song_idx);
                    hovered.map(|i| i + range.start)
                }
                Position::StartOfQueue => hovered,
                Position::EndOfQueue => hovered.map(|i| i + queue_len),
                Position::Replace => hovered,
            },
            (AutoplayKind::HoveredOrFirst, curr, hovered) => match self.position {
                Position::AfterCurrentSong => {
                    let Some(current_song_idx) = curr else {
                        bail!("No current song to queue after");
                    };

                    hovered.map(|i| i + 1 + current_song_idx).or(Some(current_song_idx + 1))
                }
                Position::BeforeCurrentSong => {
                    let Some(current_song_idx) = curr else {
                        bail!("No current song to queue before");
                    };
                    hovered.map(|i| i + current_song_idx).or(Some(current_song_idx))
                }
                Position::AfterCurrentAlbum => {
                    let Some(current_song_idx) = curr else {
                        bail!("No current song to queue after its album");
                    };

                    let range = find_range_or_bail!(current_song_idx);
                    hovered.map(|i| i + range.end).or(Some(range.end))
                }
                Position::BeforeCurrentAlbum => {
                    let Some(current_song_idx) = curr else {
                        bail!("No current song to queue before its album");
                    };

                    let range = find_range_or_bail!(current_song_idx);
                    hovered.map(|i| i + range.start).or(Some(range.start))
                }
                Position::StartOfQueue => hovered.or(Some(0)),
                Position::EndOfQueue => hovered.map(|i| i + queue_len).or(Some(queue_len)),
                Position::Replace => hovered.or(Some(0)),
            },
            (AutoplayKind::None, _, _) => None,
        })
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum RateKind {
    Modal {
        #[serde(default = "crate::config::defaults::rating_options")]
        values: Vec<i32>,
        #[serde(default = "crate::config::defaults::bool::<true>")]
        custom: bool,
        #[serde(default = "crate::config::defaults::bool::<true>")]
        like: bool,
    },
    Value(i32),
    Like(),
    Dislike(),
    Neutral(),
}

impl Default for RateKind {
    fn default() -> Self {
        RateKind::Modal { values: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10], custom: true, like: true }
    }
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq)]
pub enum DuplicateStrategy {
    All,
    None,
    NonDuplicate,
    #[default]
    Ask,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum SaveKind {
    Modal {
        #[serde(default = "crate::config::defaults::bool::<false>")]
        all: bool,
        #[serde(default)]
        duplicates_strategy: DuplicateStrategy,
    },
    Playlist {
        name: String,
        #[serde(default = "crate::config::defaults::bool::<false>")]
        all: bool,
        #[serde(default)]
        duplicates_strategy: DuplicateStrategy,
    },
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum DeleteKind {
    Modal {
        #[serde(default = "crate::config::defaults::bool::<false>")]
        all: bool,
        #[serde(default = "crate::config::defaults::bool::<true>")]
        confirmation: bool,
    },
    Playlist {
        name: String,
        #[serde(default = "crate::config::defaults::bool::<false>")]
        all: bool,
        #[serde(default = "crate::config::defaults::bool::<true>")]
        confirmation: bool,
    },
}

impl Default for SaveKind {
    fn default() -> Self {
        SaveKind::Modal { all: false, duplicates_strategy: DuplicateStrategy::default() }
    }
}

impl Default for DeleteKind {
    fn default() -> Self {
        DeleteKind::Modal { all: false, confirmation: true }
    }
}

// Common actions

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub enum CommonActionFile {
    Down,
    Up,
    Right,
    Left,
    PaneDown,
    PaneUp,
    PaneRight,
    PaneLeft,
    MoveDown,
    MoveUp,
    DownHalf,
    UpHalf,
    PageUp,
    PageDown,
    Top,
    Bottom,
    EnterSearch,
    NextResult,
    PreviousResult,
    Select,
    InvertSelection,
    Delete,
    Rename,
    Close,
    Confirm,
    FocusInput,
    Add,
    AddAll,
    AddReplace,
    AddAllReplace,
    Insert,
    InsertAll,
    AddOptions {
        #[serde(default)]
        kind: AddKind,
    },
    ShowInfo,
    ContextMenu {},
    Rate {
        #[serde(default)]
        kind: RateKind,
        #[serde(default)]
        current: bool,
        #[serde(default = "crate::config::defaults::i32::<0>")]
        min_rating: i32,
        #[serde(default = "crate::config::defaults::i32::<10>")]
        max_rating: i32,
    },
    Save {
        #[serde(default)]
        kind: SaveKind,
    },
    DeleteFromPlaylist {
        #[serde(default)]
        kind: DeleteKind,
    },
}

#[derive(Debug, Display, Clone, EnumDiscriminants, PartialEq)]
#[strum_discriminants(derive(VariantArray))]
pub enum CommonAction {
    Down,
    Up,
    Right,
    Left,
    PaneDown,
    PaneUp,
    PaneRight,
    PaneLeft,
    MoveDown,
    MoveUp,
    DownHalf,
    UpHalf,
    PageUp,
    PageDown,
    Top,
    Bottom,
    EnterSearch,
    NextResult,
    PreviousResult,
    Select,
    InvertSelection,
    Delete,
    Rename,
    Close,
    Confirm,
    FocusInput,
    #[strum(to_string = "AddOptions({kind})")]
    AddOptions {
        kind: AddKind,
    },
    ShowInfo,
    ContextMenu,
    Rate {
        kind: RateKind,
        current: bool,
        min_rating: i32,
        max_rating: i32,
    },
    Save {
        kind: SaveKind,
    },
    DeleteFromPlaylist {
        kind: DeleteKind,
    },
}

impl ToDescription for CommonAction {
    fn to_description(&self) -> Cow<'static, str> {
        match self {
            CommonAction::Up => "Go up".into(),
            CommonAction::Down => "Go down".into(),
            CommonAction::UpHalf => "Jump by half a screen up".into(),
            CommonAction::DownHalf => "Jump by half a screen down".into(),
            CommonAction::PageUp => "Jump a screen up".into(),
            CommonAction::PageDown => "Jump a screen down".into(),
            CommonAction::MoveUp => "Move current item up, for example song in a queue".into(),
            CommonAction::MoveDown => "Move current item down, for example song in a queue".into(),
            CommonAction::Right => "Go right".into(),
            CommonAction::Left => "Go left".into(),
            CommonAction::Top => "Jump all the way to the top".into(),
            CommonAction::Bottom => "Jump all the way to the bottom".into(),
            CommonAction::EnterSearch => "Enter search mode".into(),
            CommonAction::NextResult => "When a filter is active, jump to the next result".into(),
            CommonAction::PreviousResult => "When a filter is active, jump to the previous result".into(),
            CommonAction::Select => {
                "Mark current item as selected in the browser, useful for example when you want to add multiple songs to a playlist".into()
            }
            CommonAction::InvertSelection => "Inverts the current selected items".into(),
            CommonAction::Delete => {
                "Delete. For example a playlist, song from a playlist or wipe the current queue".into()
            }
            CommonAction::Rename => "Rename. Currently only for playlists".into(),
            CommonAction::Close => {
                "Close/Stop whatever action is currently going on. Cancel filter, close a modal, etc.".into()
            }
            CommonAction::Confirm => {
                "Confirm whatever action is currently going on. In browser panes it either enters a directory or adds and plays a song under cursor".into()
            }
            CommonAction::FocusInput => {
                "Focuses textbox if any is on the screen and is not focused".into()
            }
            CommonAction::PaneDown => "Focus the pane below the current one".into(),
            CommonAction::PaneUp => "Focus the pane above the current one".into(),
            CommonAction::PaneRight => "Focus the pane to the right of the current one".into(),
            CommonAction::PaneLeft => "Focus the pane to the left of the current one".into(),
            CommonAction::AddOptions { kind: AddKind::Modal(items) } => format!("Open add menu modal with {} options", items.len()).into(),
            CommonAction::AddOptions { kind: AddKind::Action(opts) } => {
                let mut buf = String::from("Add");
                if opts.all {
                    buf.push_str(" all items");
                } else {
                    buf.push_str(" item");
                }
                buf.push_str(match opts.position {
                    Position::AfterCurrentSong => " after the current song",
                    Position::BeforeCurrentSong => " before the current song",
                    Position::AfterCurrentAlbum => " after the current album",
                    Position::BeforeCurrentAlbum => " before the current album",
                    Position::StartOfQueue => " at the start of the queue",
                    Position::EndOfQueue => " at the end of the queue",
                    Position::Replace => " and replace the queue",
                });

                buf.push_str(match opts.autoplay {
                    AutoplayKind::First => " and play the first item",
                    AutoplayKind::Hovered => " and play the hovered item",
                    AutoplayKind::HoveredOrFirst => " and play hovered item or first if no song is hovered",
                    AutoplayKind::None => "",
                });

                buf.into()
            },
            CommonAction::ShowInfo => "Show info about item under cursor in a modal popup".into(),
            CommonAction::ContextMenu => "Show context menu".into(),
            CommonAction::Rate { kind: RateKind::Modal { .. }, current, .. } => {
                let mut buf = String::from("Open a modal popup with song rating options");
                if *current {
                    buf.push_str(" for the currently playing song");
                }
                buf.into()
            },
            CommonAction::Rate { kind: RateKind::Value(val), current, ..  } => {
                if *current {
                    format!("Set currently playing song's rating to {val}")
                } else {
                    format!("Set song rating to {val}")
                }.into()
            }
            CommonAction::Rate { kind: k @ RateKind::Like() | k @ RateKind::Dislike() | k @ RateKind::Neutral(), current , .. } => {
                let mut buf = String::from("Set the ");
                if *current {
                    buf.push_str("currently playing song's");
                } else {
                    buf.push_str("song's under the cursor");
                }
                buf.push_str(" like state to ");
                match k {
                    RateKind::Like() => buf.push_str("like"),
                    RateKind::Dislike() => buf.push_str("dislike"),
                    RateKind::Neutral() => buf.push_str("neutral"),
                    _ => {}
                }

                buf.into()
            },
            CommonAction::Save { kind: SaveKind::Modal { all, duplicates_strategy } } => {
                let mut buf = String::from("Open a modal popup with options to save ");
                if *all {
                    buf.push_str("all items");
                } else {
                    buf.push_str("the item under cursor");
                }
                buf.push_str(" to either a new or existing playlist. ");

                match duplicates_strategy {
                    DuplicateStrategy::All => buf.push_str("All songs will all be added"),
                    DuplicateStrategy::None => buf.push_str("No songs will be added"),
                    DuplicateStrategy::NonDuplicate => buf.push_str("Only non-duplicate songs will be added"),
                    DuplicateStrategy::Ask => buf.push_str("A modal asking what to do will be shown"),
                }
                buf.push_str(" if any songs already exist in the target playlist.");

                buf.into()
            },
            CommonAction::Save { kind: SaveKind::Playlist { name, all, duplicates_strategy } } => {
                let mut buf = String::from("Save ");
                if *all {
                    buf.push_str("all items");
                } else {
                    buf.push_str("the item under cursor");
                }

                write!(buf, " to playlist '{name}'. ").expect("Write to string buf should never fail.");

                match duplicates_strategy {
                    DuplicateStrategy::All => buf.push_str("All songs will all be added"),
                    DuplicateStrategy::None => buf.push_str("No songs will be added"),
                    DuplicateStrategy::NonDuplicate => buf.push_str("Only non-duplicate songs will be added"),
                    DuplicateStrategy::Ask => buf.push_str("A modal asking what to do will be shown"),
                }
                buf.push_str(" if any songs already exist in the target playlist.");

                buf.into()
            }
            CommonAction::DeleteFromPlaylist { kind: DeleteKind::Modal { all, confirmation } } => {
                let mut buf = String::from("Open a modal popup to delete ");
                if *all {
                    buf.push_str("all items");
                } else {
                    buf.push_str("the item under cursor");
                }
                if *confirmation {
                    buf.push_str(" with a confirmation");
                } else {
                    buf.push_str(" without confirmation");
                }
                buf.push_str(" from the selected playlist.");

                buf.into()
            }
            CommonAction::DeleteFromPlaylist { kind: DeleteKind::Playlist { name ,all, confirmation } } => {
                let mut buf = String::from("Delete ");
                if *all {
                    buf.push_str("all items");
                } else {
                    buf.push_str("the item under cursor");
                }

                write!(buf, " from playlist '{name}'. ").expect("Write to string buf should never fail.");

                if *confirmation {
                    buf.push_str("With a confirmation.");
                } else {
                    buf.push_str("Without confirmation.");
                }

                buf.into()
            }
        }
    }
}

impl TryFrom<CommonActionFile> for CommonAction {
    type Error = anyhow::Error;

    fn try_from(value: CommonActionFile) -> Result<Self, Self::Error> {
        Ok(match value {
            CommonActionFile::Up => CommonAction::Up,
            CommonActionFile::Down => CommonAction::Down,
            CommonActionFile::UpHalf => CommonAction::UpHalf,
            CommonActionFile::DownHalf => CommonAction::DownHalf,
            CommonActionFile::PageUp => CommonAction::PageUp,
            CommonActionFile::PageDown => CommonAction::PageDown,
            CommonActionFile::MoveUp => CommonAction::MoveUp,
            CommonActionFile::MoveDown => CommonAction::MoveDown,
            CommonActionFile::Right => CommonAction::Right,
            CommonActionFile::Left => CommonAction::Left,
            CommonActionFile::Top => CommonAction::Top,
            CommonActionFile::Bottom => CommonAction::Bottom,
            CommonActionFile::EnterSearch => CommonAction::EnterSearch,
            CommonActionFile::NextResult => CommonAction::NextResult,
            CommonActionFile::PreviousResult => CommonAction::PreviousResult,
            CommonActionFile::Select => CommonAction::Select,
            CommonActionFile::InvertSelection => CommonAction::InvertSelection,
            CommonActionFile::Add => CommonAction::AddOptions {
                kind: AddKind::Action(AddOpts {
                    autoplay: AutoplayKind::None,
                    position: Position::EndOfQueue,
                    all: false,
                }),
            },
            CommonActionFile::AddReplace => CommonAction::AddOptions {
                kind: AddKind::Action(AddOpts {
                    autoplay: AutoplayKind::None,
                    position: Position::Replace,
                    all: false,
                }),
            },
            CommonActionFile::Insert => CommonAction::AddOptions {
                kind: AddKind::Action(AddOpts {
                    autoplay: AutoplayKind::None,
                    position: Position::AfterCurrentSong,
                    all: false,
                }),
            },
            CommonActionFile::InsertAll => CommonAction::AddOptions {
                kind: AddKind::Action(AddOpts {
                    autoplay: AutoplayKind::None,
                    position: Position::AfterCurrentSong,
                    all: true,
                }),
            },
            CommonActionFile::AddAll => CommonAction::AddOptions {
                kind: AddKind::Action(AddOpts {
                    autoplay: AutoplayKind::None,
                    position: Position::EndOfQueue,
                    all: true,
                }),
            },
            CommonActionFile::AddAllReplace => CommonAction::AddOptions {
                kind: AddKind::Action(AddOpts {
                    autoplay: AutoplayKind::None,
                    position: Position::Replace,
                    all: true,
                }),
            },
            CommonActionFile::Delete => CommonAction::Delete,
            CommonActionFile::Rename => CommonAction::Rename,
            CommonActionFile::Close => CommonAction::Close,
            CommonActionFile::Confirm => CommonAction::Confirm,
            CommonActionFile::FocusInput => CommonAction::FocusInput,
            CommonActionFile::PaneUp => CommonAction::PaneUp,
            CommonActionFile::PaneDown => CommonAction::PaneDown,
            CommonActionFile::PaneLeft => CommonAction::PaneLeft,
            CommonActionFile::PaneRight => CommonAction::PaneRight,
            CommonActionFile::ShowInfo => CommonAction::ShowInfo,
            CommonActionFile::AddOptions { kind } => CommonAction::AddOptions { kind },
            CommonActionFile::ContextMenu {} => CommonAction::ContextMenu,
            CommonActionFile::Rate { kind, current, min_rating, max_rating } => {
                match &kind {
                    RateKind::Modal { values, custom, like } => {
                        if values.is_empty() && !custom && !like {
                            bail!(
                                "At least one of 'values', 'custom' or 'like' must be set for rating modal"
                            );
                        }

                        if !values.is_empty() {
                            if let Some(min) = values.iter().min()
                                && *min < min_rating
                            {
                                bail!("Rating must be at least {min_rating}");
                            }
                            if let Some(max) = values.iter().max()
                                && *max > max_rating
                            {
                                bail!("Rating must be at most {max_rating}");
                            }
                        }
                    }
                    RateKind::Value(v) => {
                        if *v < min_rating || *v > max_rating {
                            bail!("Rating must be between {min_rating} and {max_rating}");
                        }
                    }
                    _ => {}
                }
                CommonAction::Rate { kind, current, min_rating, max_rating }
            }
            CommonActionFile::Save { kind } => CommonAction::Save { kind },
            CommonActionFile::DeleteFromPlaylist { kind } => {
                CommonAction::DeleteFromPlaylist { kind }
            }
        })
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub enum SearchActionsFile {}

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq)]
pub enum SearchActions {}

impl ToDescription for SearchActions {
    fn to_description(&self) -> Cow<'static, str> {
        "".into()
    }
}

impl From<SearchActionsFile> for SearchActions {
    fn from(_value: SearchActionsFile) -> Self {
        unreachable!()
    }
}
