use rmpc_mpd::commands::{Status as MpdStatus, status::State as MpdState, volume::Bound};
use serde::{Deserialize, Serialize};

use crate::lua::lualib::mpd::types::OnOffOneshot;

#[derive(Debug, Serialize, Clone)]
pub struct Status {
    pub partition: String,
    pub volume: u32,
    pub repeat: bool,
    pub random: bool,
    pub single: OnOffOneshot,
    pub consume: OnOffOneshot,
    pub playlist: Option<u32>,
    pub playlist_length: u32,
    pub state: State,
    pub song: Option<usize>,
    pub song_id: Option<u32>,
    pub next_song: Option<u32>,
    pub next_song_id: Option<u32>,
    pub elapsed: u128,
    pub duration: u128,
    pub bitrate: Option<u32>,
    pub xfade: Option<u32>,
    pub mix_ramp_db: Option<String>,
    pub mix_ramp_delay: Option<String>,
    pub audio: Option<String>,
    pub updating_db: Option<u32>,
    pub error: Option<String>,
    pub last_loaded_playlist: Option<String>,
}

impl From<MpdStatus> for Status {
    fn from(value: MpdStatus) -> Self {
        Self {
            partition: value.partition,
            volume: *value.volume.value(),
            repeat: value.repeat,
            random: value.random,
            single: value.single.into(),
            consume: value.consume.into(),
            playlist: value.playlist,
            playlist_length: value.playlistlength,
            state: value.state.into(),
            song: value.song,
            song_id: value.songid,
            next_song: value.nextsong,
            next_song_id: value.nextsongid,
            elapsed: value.elapsed.as_millis(),
            duration: value.duration.as_millis(),
            bitrate: value.bitrate,
            xfade: value.xfade,
            mix_ramp_db: value.mixrampdb,
            mix_ramp_delay: value.mixrampdelay,
            audio: value.audio,
            updating_db: value.updating_db,
            error: value.error,
            last_loaded_playlist: value.lastloadedplaylist,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Play,
    Stop,
    Pause,
}

impl From<MpdState> for State {
    fn from(value: MpdState) -> Self {
        match value {
            MpdState::Play => State::Play,
            MpdState::Stop => State::Stop,
            MpdState::Pause => State::Pause,
        }
    }
}

impl From<State> for MpdState {
    fn from(value: State) -> Self {
        match value {
            State::Play => MpdState::Play,
            State::Stop => MpdState::Stop,
            State::Pause => MpdState::Pause,
        }
    }
}
