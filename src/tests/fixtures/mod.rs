use ratatui::{backend::TestBackend, Terminal};
use rstest::fixture;

use crate::{config::Config, context::AppContext, mpd::commands::Status};

pub mod mpd_client;

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

#[fixture]
#[allow(clippy::unwrap_used)]
pub fn terminal() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(100, 100)).unwrap()
}
