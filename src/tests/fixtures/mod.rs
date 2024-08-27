use rstest::fixture;

use crate::{config::Config, context::AppContext, mpd::commands::Status};

pub mod mpd_client;

#[fixture]
pub fn state() -> AppContext {
    AppContext::default()
}

#[fixture]
pub fn status() -> Status {
    Status::default()
}

#[fixture]
pub fn app_context() -> AppContext {
    AppContext::default()
}

#[fixture]
pub fn config() -> Config {
    Config::default()
}
