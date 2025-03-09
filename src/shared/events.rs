use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::KeyEvent;
use serde::{Deserialize, Serialize};

use super::{
    lrc::{LrcIndex, LrcIndexEntry},
    mouse_event::MouseEvent,
    mpd_query::{MpdCommand, MpdQuery, MpdQueryResult, MpdQuerySync},
};
use crate::{
    config::{Config, cli::Command, tabs::PaneType, theme::UiConfig},
    mpd::commands::IdleEvent,
    ui::UiAppEvent,
};

#[derive(Debug)]
#[allow(unused)]
pub(crate) enum ClientRequest {
    Query(MpdQuery),
    QuerySync(MpdQuerySync),
    Command(MpdCommand),
}

#[derive(Debug)]
#[allow(unused)]
pub(crate) enum WorkRequest {
    IndexLyrics {
        lyrics_dir: String,
    },
    IndexSingleLrc {
        /// Absolute path to the lrc file
        path: PathBuf,
    },
    Command(Command),
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // the instances are short lived events, its fine.
pub(crate) enum WorkDone {
    LyricsIndexed { index: LrcIndex },
    SingleLrcIndexed { lrc_entry: Option<LrcIndexEntry> },
    MpdCommandFinished { id: &'static str, target: Option<PaneType>, data: MpdQueryResult },
    None,
}

#[derive(Debug)]
pub(crate) enum AppEvent {
    UserKeyInput(KeyEvent),
    UserMouseInput(MouseEvent),
    Status(String, Level),
    Log(Vec<u8>),
    IdleEvent(IdleEvent),
    RequestRender,
    Resized { columns: u16, rows: u16 },
    ResizedDebounced { columns: u16, rows: u16 },
    WorkDone(Result<WorkDone>),
    UiEvent(UiAppEvent),
    Reconnected,
    LostConnection,
    TmuxHook { hook: String },
    ConfigChanged { config: Config, keep_old_theme: bool },
    ThemeChanged { theme: UiConfig },
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Eq, Hash, PartialEq)]
#[allow(dead_code)]
pub enum Level {
    Trace,
    Debug,
    Warn,
    Error,
    Info,
}
