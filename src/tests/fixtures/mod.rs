use std::{cell::Cell, collections::HashSet};

use crossbeam::channel::{Receiver, Sender, unbounded};
use ratatui::{Terminal, backend::TestBackend};
use rstest::fixture;

use crate::{
    config::{Config, ConfigFile, tabs::TabName},
    context::AppContext,
    core::scheduler::Scheduler,
    mpd::commands::Status,
    shared::{
        events::{ClientRequest, WorkRequest},
        lrc::LrcIndex,
        ring_vec::RingVec,
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
    let config = ConfigFile::default()
        .into_config(None, None, None, None, true)
        .expect("Test default config to convert correctly");

    let chan1 = Box::leak(Box::new(chan1));
    let scheduler = Scheduler::new((chan1.0.clone(), unbounded().0));
    AppContext {
        status: Status::default(),
        config: std::sync::Arc::new(config),
        queue: Vec::default(),
        active_tab: TabName::from("test_tab"),
        app_event_sender: chan1.0.clone(),
        work_sender: work_request_channel.0.clone(),
        client_request_sender: client_request_channel.0.clone(),
        supported_commands: HashSet::new(),
        needs_render: Cell::new(false),
        lrc_index: LrcIndex::default(),
        should_fetch_stickers: false,
        rendered_frames: 0,
        scheduler,
        db_update_start: None,
        messages: RingVec::default(),
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
