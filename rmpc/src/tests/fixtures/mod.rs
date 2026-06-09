use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use crossbeam::channel::{Receiver, Sender, unbounded};
use ratatui::{Terminal, backend::TestBackend};
use rmpc_mpd::commands::Status;
use rmpc_shared::version::Version;
use rstest::fixture;

use crate::{
    config::{Config, ConfigFile, tabs::TabName, theme::UiConfigFile},
    core::scheduler::Scheduler,
    ctx::{Ctx, StickersSupport},
    shared::{
        events::{AppEvent, ClientRequest, WorkRequest},
        ipc::ipc_stream::IpcStream,
        keys::KeyResolver,
        lrc::LrcIndex,
        ring_vec::RingVec,
        ytdlp::YtDlpManager,
    },
    ui::input::InputManager,
};

/// Tests must run against a stable, neutral theme/config — not the real
/// (Refined) default, which would change expected formatting output. This
/// builds a `Config` from the hand-built bare defaults.
fn bare_config() -> Config {
    let ui = UiConfigFile::bare_default().try_into().expect("bare UiConfig should convert");
    ConfigFile::bare_default()
        .into_config(ui, None, None, true)
        .expect("bare Config should be valid")
}

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
pub fn app_event_channel() -> (Sender<AppEvent>, Receiver<AppEvent>) {
    unbounded()
}

#[fixture]
pub fn ctx(
    app_event_channel: (Sender<AppEvent>, Receiver<AppEvent>),
    work_request_channel: (Sender<WorkRequest>, Receiver<WorkRequest>),
    client_request_channel: (Sender<ClientRequest>, Receiver<ClientRequest>),
) -> Ctx {
    let config = bare_config();

    let scheduler = Scheduler::new((app_event_channel.0.clone(), unbounded().0));
    let key_resolver = KeyResolver::new(&config);
    Box::leak(Box::new(app_event_channel.1.clone()));
    Ctx {
        ytdlp_manager: YtDlpManager::new(work_request_channel.0.clone()),
        mpd_version: Version::new(1, 0, 0),
        status: Status::default(),
        config: std::sync::Arc::new(config),
        queue: Vec::default(),
        stickers: HashMap::new(),
        active_tab: TabName::from("test_tab"),
        app_event_sender: app_event_channel.0.clone(),
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
        key_resolver,
        cached_queue_time_total: Duration::default(),
        current_song: None,
    }
}

#[fixture]
pub fn config() -> Config {
    bare_config()
}

#[fixture]
#[allow(clippy::unwrap_used)]
pub fn terminal() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(100, 100)).unwrap()
}
