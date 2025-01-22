use anyhow::Result;
use crossterm::event::KeyEvent;

use super::{
    lrc::LrcIndex,
    mouse_event::MouseEvent,
    mpd_query::{MpdCommand, MpdQuery, MpdQueryResult, MpdQuerySync},
};
use crate::{
    config::{cli::Command, tabs::PaneType},
    mpd::commands::IdleEvent,
    ui::{Level, UiAppEvent},
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
    IndexLyrics { lyrics_dir: &'static str },
    Command(Command),
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // the instances are short lived events, its fine.
pub(crate) enum WorkDone {
    LyricsIndexed { index: LrcIndex },
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
    WorkDone(Result<WorkDone>),
    UiEvent(UiAppEvent),
    Reconnected,
    LostConnection,
}
