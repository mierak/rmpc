use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    os::unix::net::UnixStream,
    time::Instant,
};

use crossbeam::channel::{Receiver, Sender, unbounded};
use ratatui::{Terminal, backend::TestBackend};
use rstest::fixture;

use crate::{
    config::{Config, ConfigFile, tabs::TabName},
    core::scheduler::Scheduler,
    ctx::{Ctx, StickersSupport},
    mpd::{commands::Status, version::Version},
    shared::{
        events::{ClientRequest, WorkRequest},
        ipc::ipc_stream::IpcStream,
        lrc::LrcIndex,
        ring_vec::RingVec,
    },
    ui::input::InputManager,
};

pub mod mpd_client;

#[fixture]
pub fn ipc_stream() -> IpcStream {
    let pair = UnixStream::pair().expect("UnixStream pair should not fail");
    pair.0.into()
}

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
pub fn ctx(
    work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
    client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
) -> Ctx {
    let chan1 = unbounded();
    let config = ConfigFile::default()
        .into_config(None, None, None, None, true)
        .expect("Test default config to convert correctly");

    let chan1 = Box::leak(Box::new(chan1));
    let scheduler = Scheduler::new((chan1.0.clone(), unbounded().0));
    Ctx {
        mpd_version: Version::new(1, 0, 0),
        status: Status::default(),
        config: std::sync::Arc::new(config),
        queue: Vec::default(),
        stickers: HashMap::new(),
        active_tab: TabName::from("test_tab"),
        app_event_sender: chan1.0.clone(),
        work_sender: work_request_channel.0.clone(),
        client_request_sender: client_request_channel.0.clone(),
        supported_commands: HashSet::new(),
        needs_render: Cell::new(false),
        stickers_to_fetch: RefCell::new(HashSet::new()),
        lrc_index: LrcIndex::default(),
        stickers_supported: StickersSupport::Unsupported,
        rendered_frames: 0,
        scheduler,
        db_update_start: None,
        messages: RingVec::default(),
        last_status_update: Instant::now(),
        song_played: None,
        input: InputManager::default(),
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
