use std::{cell::Cell, collections::HashSet, sync::mpsc::channel};

use ratatui::{backend::TestBackend, Terminal};
use rstest::fixture;

use crate::{
    config::{Config, ConfigFile, Leak},
    context::AppContext,
    mpd::commands::Status,
    shared::lrc::LrcIndex,
};

pub mod mpd_client;

#[fixture]
pub fn status() -> Status {
    Status::default()
}

#[fixture]
pub fn app_context() -> AppContext {
    let chan1 = channel();
    let chan2 = channel();
    chan1.1.leak();
    chan2.1.leak();
    let config = ConfigFile::default()
        .into_config(None, None, None, true)
        .expect("Test default config to convert correctly")
        .leak();
    AppContext {
        status: Status::default(),
        config,
        queue: Vec::default(),
        app_event_sender: chan1.0,
        work_sender: chan2.0,
        supported_commands: HashSet::new(),
        needs_render: Cell::new(false),
        lrc_index: LrcIndex::default(),
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
