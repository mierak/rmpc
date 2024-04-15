use rstest::fixture;

use crate::state::State;

pub mod mpd_client;

#[fixture]
pub fn state() -> State {
    State::default()
}
