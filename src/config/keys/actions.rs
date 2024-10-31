use itertools::Itertools;
use strum::Display;

use crate::config::{tabs::TabName, utils::tilde_expand};

use super::ToDescription;

// Global actions

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy)]
pub enum GlobalAction {
    Quit,
    ShowHelp,
    ShowCurrentSongInfo,
    ShowOutputs,
    NextTrack,
    PreviousTrack,
    Stop,
    ToggleRepeat,
    ToggleSingle,
    ToggleRandom,
    ToggleConsume,
    TogglePause,
    VolumeUp,
    VolumeDown,
    SeekForward,
    SeekBack,
    CommandMode,
    NextTab,
    PreviousTab,
    SwitchToTab(TabName),
    Command {
        command: &'static str,
        description: Option<&'static str>,
    },
    ExternalCommand {
        command: &'static [&'static str],
        description: Option<&'static str>,
    },
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, Clone, Ord, PartialOrd)]
pub enum GlobalActionFile {
    Quit,
    ShowHelp,
    ShowCurrentSongInfo,
    ShowOutputs,
    NextTrack,
    PreviousTrack,
    Stop,
    ToggleRepeat,
    ToggleSingle,
    ToggleRandom,
    ToggleConsume,
    TogglePause,
    VolumeUp,
    VolumeDown,
    SeekForward,
    SeekBack,
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
}

impl From<GlobalActionFile> for GlobalAction {
    fn from(value: GlobalActionFile) -> Self {
        match value {
            GlobalActionFile::Quit => GlobalAction::Quit,
            GlobalActionFile::ShowOutputs => GlobalAction::ShowOutputs,
            GlobalActionFile::ShowCurrentSongInfo => GlobalAction::ShowCurrentSongInfo,
            GlobalActionFile::CommandMode => GlobalAction::CommandMode,
            GlobalActionFile::Command { command, description } => GlobalAction::Command {
                command: command.leak(),
                description: description.map(|s| s.leak() as &'static str),
            },
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
            GlobalActionFile::VolumeDown => GlobalAction::VolumeDown,
            GlobalActionFile::VolumeUp => GlobalAction::VolumeUp,
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
            GlobalActionFile::ExternalCommand { command, description } => GlobalAction::ExternalCommand {
                command: command
                    .into_iter()
                    .map(|v| tilde_expand(&v).into_owned().leak() as &'static str)
                    .collect_vec()
                    .leak(),
                description: description.map(|s| s.leak() as &'static str),
            },
        }
    }
}

impl ToDescription for GlobalAction {
    fn to_description(&self) -> &str {
        match self {
            GlobalAction::Quit => "Exit rmpc",
            GlobalAction::ShowOutputs => "Show MPD outputs config",
            GlobalAction::ShowCurrentSongInfo => "Show metadata of the currently playing song in a modal popup",
            GlobalAction::ToggleRepeat => "Toggle repeat",
            GlobalAction::ToggleSingle => {
                "Whether to stop playing after single track or repeat track/playlist when repeat is on"
            }
            GlobalAction::ToggleRandom => "Toggles random playback",
            GlobalAction::ToggleConsume => "Remove song from the queue after playing",
            GlobalAction::TogglePause => "Pause/Unpause playback",
            GlobalAction::Stop => "Stop playback",
            GlobalAction::VolumeUp => "Raise volume",
            GlobalAction::VolumeDown => "Lower volume",
            GlobalAction::NextTrack => "Play next track in the queue",
            GlobalAction::PreviousTrack => "Play previous track in the queue",
            GlobalAction::SeekForward => "Seek currently playing track forwards",
            GlobalAction::SeekBack => "Seek currently playing track backwards",
            GlobalAction::NextTab => "Switch to next tab",
            GlobalAction::PreviousTab => "Switch to previous tab",
            GlobalAction::SwitchToTab(TabName("Queue")) => "Switch directly to Queue tab",
            GlobalAction::SwitchToTab(TabName("Directories")) => "Switch directly to Directories tab",
            GlobalAction::SwitchToTab(TabName("Artists")) => "Switch directly to Artists tab",
            GlobalAction::SwitchToTab(TabName("Albums")) => "Switch directly to Albums tab",
            GlobalAction::SwitchToTab(TabName("Playlists")) => "Switch directly to Playlists tab",
            GlobalAction::SwitchToTab(TabName("Search")) => "Switch directly to Search tab",
            GlobalAction::SwitchToTab(name) => format!("Switch directly to {name} tab").leak(),
            GlobalAction::ShowHelp => "Show keybinds",
            GlobalAction::CommandMode => "Enter command mode",
            GlobalAction::Command { description: None, .. } => "Execute a command",
            GlobalAction::Command {
                description: Some(desc),
                ..
            } => desc,
            GlobalAction::ExternalCommand { description: None, .. } => "Execute an external command",
            GlobalAction::ExternalCommand {
                description: Some(desc),
                ..
            } => desc,
        }
    }
}

// Albums actions

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum AlbumsActionsFile {}

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy)]
pub enum AlbumsActions {}

impl From<AlbumsActionsFile> for AlbumsActions {
    fn from(_value: AlbumsActionsFile) -> Self {
        unreachable!()
    }
}

impl ToDescription for AlbumsActions {
    fn to_description(&self) -> &str {
        ""
    }
}

// Artists actions

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum ArtistsActionsFile {}

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ArtistsActions {}

impl ToDescription for ArtistsActions {
    fn to_description(&self) -> &str {
        ""
    }
}

impl From<ArtistsActionsFile> for ArtistsActions {
    fn from(_value: ArtistsActionsFile) -> Self {
        unreachable!()
    }
}

// Directories actions

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum DirectoriesActionsFile {}

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy)]
pub enum DirectoriesActions {}

impl ToDescription for DirectoriesActions {
    fn to_description(&self) -> &str {
        ""
    }
}

impl From<DirectoriesActionsFile> for DirectoriesActions {
    fn from(_value: DirectoriesActionsFile) -> Self {
        unreachable!()
    }
}

// Logs actions
#[cfg(debug_assertions)]
#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum LogsActionsFile {
    Clear,
}

#[cfg(debug_assertions)]
#[allow(dead_code)]
#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy)]
pub enum LogsActions {
    Clear,
}

#[cfg(debug_assertions)]
impl From<LogsActionsFile> for LogsActions {
    fn from(value: LogsActionsFile) -> Self {
        match value {
            LogsActionsFile::Clear => LogsActions::Clear,
        }
    }
}

#[cfg(debug_assertions)]
impl ToDescription for LogsActions {
    fn to_description(&self) -> &str {
        match self {
            LogsActions::Clear => "Clear logs",
        }
    }
}

// Queue actions

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum QueueActionsFile {
    Delete,
    DeleteAll,
    Play,
    Save,
    AddToPlaylist,
    ShowInfo,
    JumpToCurrent,
}

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy)]
pub enum QueueActions {
    Delete,
    DeleteAll,
    Play,
    Save,
    AddToPlaylist,
    ShowInfo,
    JumpToCurrent,
}

impl From<QueueActionsFile> for QueueActions {
    fn from(value: QueueActionsFile) -> Self {
        match value {
            QueueActionsFile::Delete => QueueActions::Delete,
            QueueActionsFile::DeleteAll => QueueActions::DeleteAll,
            QueueActionsFile::Play => QueueActions::Play,
            QueueActionsFile::Save => QueueActions::Save,
            QueueActionsFile::AddToPlaylist => QueueActions::AddToPlaylist,
            QueueActionsFile::ShowInfo => QueueActions::ShowInfo,
            QueueActionsFile::JumpToCurrent => QueueActions::JumpToCurrent,
        }
    }
}

impl ToDescription for QueueActions {
    fn to_description(&self) -> &str {
        match self {
            QueueActions::Delete => "Remove song under curor from the queue",
            QueueActions::DeleteAll => "Clear current queue",
            QueueActions::Play => "Play song under cursor",
            QueueActions::Save => "Save current queue as a new playlist",
            QueueActions::AddToPlaylist => "Add song under cursor to an existing playlist",
            QueueActions::ShowInfo => "Show metadata of the song under cursor in a modal popup",
            QueueActions::JumpToCurrent => "Moves the cursor in Queue table to the currently playing song",
        }
    }
}

// Common actions

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, Clone, Ord, PartialOrd)]
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
    Top,
    Bottom,
    EnterSearch,
    NextResult,
    PreviousResult,
    Select,
    Add,
    Delete,
    Rename,
    Close,
    Confirm,
    FocusInput,
    AddAll,
}

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy)]
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
    Top,
    Bottom,
    EnterSearch,
    NextResult,
    PreviousResult,
    Select,
    Add,
    Delete,
    Rename,
    Close,
    Confirm,
    FocusInput,
    AddAll,
}

impl ToDescription for CommonAction {
    fn to_description(&self) -> &str {
        match self {
            CommonAction::Up => "Go up",
            CommonAction::Down => "Go down",
            CommonAction::UpHalf => "Jump by half a screen up",
            CommonAction::DownHalf => "Jump by half a screen down",
            CommonAction::MoveUp => "Move current item up, for example song in a queue",
            CommonAction::MoveDown => "Move current item down, for example song in a queue",
            CommonAction::Right => "Go right",
            CommonAction::Left => "Go left",
            CommonAction::Top => "Jump all the way to the top",
            CommonAction::Bottom => "Jump all the way to the bottom",
            CommonAction::EnterSearch => "Enter search mode",
            CommonAction::NextResult => "When a filter is active, jump to the next result",
            CommonAction::PreviousResult => "When a filter is active, jump to the previous result",
            CommonAction::Select => "Mark current item as selected in the browser, useful for example when you want to add multiple songs to a playlist",
            CommonAction::Add => "Add item to queue",
            CommonAction::AddAll => "Add all items to queue",
            CommonAction::Delete => "Delete. For example a playlist, song from a playlist or wipe the current queue",
            CommonAction::Rename => "Rename. Currently only for playlists",
            CommonAction::Close => "Close/Stop whatever action is currently going on. Cancel filter, close a modal, etc.",
            CommonAction::Confirm => "Confirm whatever action is currently going on",
            CommonAction::FocusInput => "Focuses textbox if any is on the screen and is not focused",
            CommonAction::PaneDown => "Focus the pane below the current one",
            CommonAction::PaneUp => "Focus the pane above the current one",
            CommonAction::PaneRight => "Focus the pane to the right of the current one",
            CommonAction::PaneLeft => "Focus the pane to the left of the current one",
        }
    }
}

impl From<CommonActionFile> for CommonAction {
    fn from(value: CommonActionFile) -> Self {
        match value {
            CommonActionFile::Up => CommonAction::Up,
            CommonActionFile::Down => CommonAction::Down,
            CommonActionFile::UpHalf => CommonAction::UpHalf,
            CommonActionFile::DownHalf => CommonAction::DownHalf,
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
            CommonActionFile::Add => CommonAction::Add,
            CommonActionFile::Delete => CommonAction::Delete,
            CommonActionFile::Rename => CommonAction::Rename,
            CommonActionFile::Close => CommonAction::Close,
            CommonActionFile::Confirm => CommonAction::Confirm,
            CommonActionFile::FocusInput => CommonAction::FocusInput,
            CommonActionFile::AddAll => CommonAction::AddAll,
            CommonActionFile::PaneUp => CommonAction::PaneUp,
            CommonActionFile::PaneDown => CommonAction::PaneDown,
            CommonActionFile::PaneLeft => CommonAction::PaneLeft,
            CommonActionFile::PaneRight => CommonAction::PaneRight,
        }
    }
}

// Playlist actions

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum PlaylistsActionsFile {}

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy)]
pub enum PlaylistsActions {}

impl ToDescription for PlaylistsActions {
    fn to_description(&self) -> &str {
        ""
    }
}

impl From<PlaylistsActionsFile> for PlaylistsActions {
    fn from(_value: PlaylistsActionsFile) -> Self {
        unreachable!()
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum SearchActionsFile {}

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy)]
pub enum SearchActions {}

impl ToDescription for SearchActions {
    fn to_description(&self) -> &str {
        ""
    }
}

impl From<SearchActionsFile> for SearchActions {
    fn from(_value: SearchActionsFile) -> Self {
        unreachable!()
    }
}
