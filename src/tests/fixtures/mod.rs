use std::sync::mpsc::channel;

use rstest::fixture;

use crate::{config::Config, context::AppContext, mpd::commands::Status};

pub mod mpd_client;

#[fixture]
pub fn status() -> Status {
    Status::default()
}

#[fixture]
pub fn app_context() -> AppContext {
    AppContext {
        status: Status::default(),
        config: Box::leak(Box::default()),
        queue: Vec::default(),
        app_event_sender: channel().0,
        work_sender: channel().0,
    }
}

#[fixture]
pub fn config() -> Config {
    Config::default()
}
