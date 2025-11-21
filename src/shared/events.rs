use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use crossterm::event::KeyEvent;
use serde::{Deserialize, Serialize};

use super::{
    ipc::ipc_stream::IpcStream,
    lrc::LrcIndex,
    mouse_event::MouseEvent,
    mpd_query::{MpdCommand, MpdQuery, MpdQueryResult, MpdQuerySync},
    ytdlp::{SearchItem, YtDlpHostKind},
};
use crate::{
    config::{
        Config,
        Size,
        cli::{Command, RemoteCommandQuery},
        tabs::PaneType,
        theme::UiConfig,
    },
    mpd::{QueuePosition, commands::IdleEvent},
    shared::lrc::LrcMetadata,
    ui::{UiAppEvent, image::facade::EncodeData},
};

#[derive(Debug)]
#[allow(unused)]
pub(crate) enum ClientRequest {
    Query(MpdQuery),
    QuerySync(MpdQuerySync),
    Command(MpdCommand),
}

#[allow(unused)]
pub(crate) enum WorkRequest {
    IndexLyrics {
        lyrics_dir: String,
    },
    IndexSingleLrc {
        /// Absolute path to the lrc file
        path: PathBuf,
    },
    SearchYt {
        query: String,
        kind: YtDlpHostKind,
        limit: usize,
        interactive: bool,
        position: Option<QueuePosition>,
    },
    Command(Command),
    ResizeImage(Box<dyn FnOnce() -> Result<EncodeData> + Send + Sync>),
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // the instances are short lived events, its fine.
pub(crate) enum WorkDone {
    LyricsIndexed { index: LrcIndex },
    SingleLrcIndexed { path: PathBuf, metadata: Option<LrcMetadata> },
    MpdCommandFinished { id: &'static str, target: Option<PaneType>, data: MpdQueryResult },
    SearchYtResults { items: Vec<SearchItem>, position: Option<QueuePosition> },
    ImageResized { data: Result<EncodeData> },
    None,
}

// The instances are short lived events, boxing would most likely only hurt
// here.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum AppEvent {
    UserKeyInput(KeyEvent),
    UserMouseInput(MouseEvent),
    Status(String, Level, Duration),
    InfoModal {
        message: Vec<String>,
        title: Option<String>,
        size: Option<Size>,
        replacement_id: Option<String>,
    },
    Log(Vec<u8>),
    IdleEvent(IdleEvent),
    RequestRender,
    Resized {
        columns: u16,
        rows: u16,
    },
    ResizedDebounced {
        columns: u16,
        rows: u16,
    },
    WorkDone(Result<WorkDone>),
    UiEvent(UiAppEvent),
    Reconnected,
    LostConnection,
    TmuxHook {
        hook: String,
    },
    ConfigChanged {
        config: Box<Config>,
        keep_old_theme: bool,
    },
    ThemeChanged {
        theme: Box<UiConfig>,
    },
    RemoteSwitchTab {
        tab_name: String,
    },
    IpcQuery {
        stream: IpcStream,
        targets: Vec<RemoteCommandQuery>,
    },
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
