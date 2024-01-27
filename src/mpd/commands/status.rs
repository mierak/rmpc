use std::time::Duration;

use anyhow::anyhow;

use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

use super::Volume;

#[derive(Debug, Default)]
pub struct Status {
    pub partition: String, // the name of the current partition (see Partition commands)
    pub volume: Volume,    // 0-100 (deprecated: -1 if the volume cannot be determined)
    pub repeat: bool,
    pub random: bool,
    pub single: OnOffOneshot,
    pub consume: OnOffOneshot,
    pub playlist: Option<u32>,        // 31-bit unsigned integer, the playlist version number
    pub playlistlength: u32,          // integer, the length of the playlist
    pub state: State,                 // play, stop, or pause
    pub song: Option<u32>,            // playlist song number of the current song stopped on or playing
    pub songid: Option<u32>,          // playlist songid of the current song stopped on or playing
    pub nextsong: Option<u32>,        // playlist song number of the next song to be played
    pub nextsongid: Option<u32>,      // playlist songid of the next song to be played
    pub elapsed: Duration, // Total time elapsed within the current song in seconds, but with higher resolution.
    pub duration: Duration, // Duration of the current song in seconds.
    pub bitrate: Option<u32>, // instantaneous bitrate in kbps
    pub xfade: Option<u32>, // crossfade in seconds (see Cross-Fading)
    pub mixrampdb: Option<String>, // mixramp threshold in dB
    pub mixrampdelay: Option<String>, // mixrampdelay in seconds
    pub audio: Option<String>, // The format emitted by the decoder plugin during playback, format: samplerate:bits:channels. See Global Audio Format for a detailed explanation.
    pub updating_db: Option<u32>, // job id
    pub error: Option<String>, // if there is an error, returns message here
}

impl FromMpd for Status {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "partition" => self.partition = value,
            "volume" => self.volume = Volume::new(value.parse()?),
            "repeat" => self.repeat = value != "0",
            "random" => self.random = value != "0",
            "single" => self.single = value.parse()?,
            "consume" => self.consume = value.parse()?,
            "playlist" => self.playlist = Some(value.parse()?),
            "playlistlength" => self.playlistlength = value.parse()?,
            "state" => self.state = value.parse()?,
            "song" => self.song = Some(value.parse()?),
            "songid" => self.songid = Some(value.parse()?),
            "nextsong" => self.nextsong = Some(value.parse()?),
            "nextsongid" => self.nextsongid = Some(value.parse()?),
            "elapsed" => self.elapsed = Duration::from_secs_f32(value.parse()?),
            "duration" => self.duration = Duration::from_secs_f32(value.parse()?),
            "bitrate" if value != "0" => self.bitrate = Some(value.parse()?),
            "xfade" => self.xfade = Some(value.parse()?),
            "mixrampdb" => self.mixrampdb = Some(value),
            "mixrampdelay" => self.mixrampdelay = Some(value),
            "audio" => self.audio = Some(value),
            "updating_db" => self.updating_db = Some(value.parse()?),
            "error" => self.error = Some(value),
            "time" => {} // deprecated
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }

    fn finish(self) -> Result<Self, crate::mpd::errors::MpdError> {
        Ok(self)
    }
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum State {
    Play,
    #[default]
    Stop,
    Pause,
}

#[derive(Debug, Default, Clone, Copy)]
pub enum OnOffOneshot {
    On,
    #[default]
    Off,
    Oneshot,
}

impl OnOffOneshot {
    pub fn cycle(self) -> Self {
        match self {
            OnOffOneshot::On => OnOffOneshot::Off,
            OnOffOneshot::Off => OnOffOneshot::Oneshot,
            OnOffOneshot::Oneshot => OnOffOneshot::On,
        }
    }

    pub fn cycle_pre_mpd_24(self) -> Self {
        match self {
            OnOffOneshot::On => OnOffOneshot::Off,
            OnOffOneshot::Off => OnOffOneshot::On,
            OnOffOneshot::Oneshot => OnOffOneshot::Off,
        }
    }

    pub fn to_mpd_value(self) -> &'static str {
        match self {
            OnOffOneshot::On => "1",
            OnOffOneshot::Off => "0",
            OnOffOneshot::Oneshot => "oneshot",
        }
    }
}

impl std::fmt::Display for OnOffOneshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                OnOffOneshot::On => "On",
                OnOffOneshot::Off => "Off",
                OnOffOneshot::Oneshot => "OS",
            }
        )?;
        Ok(())
    }
}

impl std::str::FromStr for OnOffOneshot {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "0" => Ok(OnOffOneshot::Off),
            "1" => Ok(OnOffOneshot::On),
            "oneshot" => Ok(OnOffOneshot::Oneshot),
            val => Err(anyhow!("Received unknown value for OnOffOneshot '{}'", val)),
        }
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                State::Play => "Playing",
                State::Stop => "Stopped",
                State::Pause => "Paused",
            }
        )
    }
}
impl std::str::FromStr for State {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "play" => Ok(Self::Play),
            "stop" => Ok(Self::Stop),
            "pause" => Ok(Self::Pause),
            _ => Err(anyhow!("Invalid State: '{}'", s)),
        }
    }
}
