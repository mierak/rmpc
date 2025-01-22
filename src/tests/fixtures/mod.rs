use std::{cell::Cell, collections::HashSet};

use crossbeam::channel::{unbounded, Receiver, Sender};
use ratatui::{backend::TestBackend, Terminal};
use rstest::fixture;

use crate::{
    config::{Config, ConfigFile, Leak},
    context::AppContext,
    mpd::commands::Status,
    shared::{
        events::{ClientRequest, WorkRequest},
        lrc::LrcIndex,
    },
};

pub mod mpd_client;

#[fixture]
pub fn status() -> Status {
    Status::default()
}

#[fixture]
pub fn work_request_channel() -> (Sender<WorkRequest>, Receiver<WorkRequest>) {
    unbounded()
}

#[fixture]
pub fn client_request_channel() -> (Sender<ClientRequest>, Receiver<ClientRequest>) {
    unbounded()
}

#[fixture]
pub fn app_context(
    work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
    client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
) -> AppContext {
    let chan1 = unbounded();
    chan1.1.leak();
    let config = ConfigFile::default()
        .into_config(None, None, None, true)
        .expect("Test default config to convert correctly")
        .leak();

    AppContext {
        status: Status::default(),
        config,
        queue: Vec::default(),
        app_event_sender: chan1.0,
        work_sender: work_request_channel.0.clone(),
        client_request_sender: client_request_channel.0.clone(),
        supported_commands: HashSet::new(),
        needs_render: Cell::new(false),
        lrc_index: LrcIndex::default(),
        should_fetch_stickers: false,
        rendered_frames: 0,
    }
}

#[fixture]
pub fn config() -> Config {
    Config::default()
}

#[fixture]
#[allow(clippy::unwrap_used)]
pub fn terminal() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(100, 100)).unwrap()
}
