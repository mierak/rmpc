use rstest::fixture;

use crate::{config::Config, mpd::commands::Status, state::State};

pub mod mpd_client;

#[fixture]
pub fn state() -> State {
    State::default()
}

#[fixture]
pub fn status() -> Status {
    Status::default()
}

#[fixture]
pub fn config() -> Config {
    Config::default()
}
