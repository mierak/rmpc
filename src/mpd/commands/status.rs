use std::time::Duration;

use anyhow::anyhow;
use anyhow::Context;

use super::Volume;

pub const COMMAND: &[u8; 6] = b"status";

#[derive(Debug, Default)]
pub struct Status {
    pub partition: String,            // the name of the current partition (see Partition commands)
    pub volume: Volume,               // 0-100 (deprecated: -1 if the volume cannot be determined)
    pub repeat: bool,                 // 0 or 1
    pub random: bool,                 // 0 or 1
    pub single: Single,               // 0, 1, or oneshot
    pub consume: String,              // 0, 1 or oneshot
    pub playlist: Option<u32>,        // 31-bit unsigned integer, the playlist version number
    pub playlistlength: u32,          // integer, the length of the playlist
    pub state: State,                 // play, stop, or pause
    pub song: Option<u32>,            // playlist song number of the current song stopped on or playing
    pub songid: Option<u32>,          // playlist songid of the current song stopped on or playing
    pub nextsong: Option<u32>,        // playlist song number of the next song to be played
    pub nextsongid: Option<u32>,      // playlist songid of the next song to be played
    pub elapsed: Duration, // Total time elapsed within the current song in seconds, but with higher resolution.
    pub duration: Duration, // Duration of the current song in seconds.
    pub bitrate: Option<String>, // instantaneous bitrate in kbps
    pub xfade: Option<u32>, // crossfade in seconds (see Cross-Fading)
    pub mixrampdb: Option<String>, // mixramp threshold in dB
    pub mixrampdelay: Option<String>, // mixrampdelay in seconds
    pub audio: Option<String>, // The format emitted by the decoder plugin during playback, format: samplerate:bits:channels. See Global Audio Format for a detailed explanation.
    pub updating_db: Option<u32>, // job id
    pub error: Option<String>, // if there is an error, returns message here
}

#[derive(Debug, Default, PartialEq)]
pub enum State {
    Play,
    #[default]
    Stop,
    Pause,
}

#[derive(Debug, Default)]
pub enum Single {
    On,
    #[default]
    Off,
    Oneshot,
}

impl Single {
    pub fn cycle(&self) -> Self {
        match self {
            Single::On => Single::Off,
            Single::Off => Single::Oneshot,
            Single::Oneshot => Single::On,
        }
    }
    pub fn to_mpd_value(&self) -> &'static str {
        match self {
            Single::On => "1",
            Single::Off => "0",
            Single::Oneshot => "oneshot",
        }
    }
}

impl std::fmt::Display for Single {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Single::On => "On",
                Single::Off => "Off",
                Single::Oneshot => "Oneshot",
            }
        )?;
        Ok(())
    }
}

impl std::str::FromStr for Single {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "0" => Single::Off,
            "1" => Single::On,
            "oneshot" => Single::Oneshot,
            _ => todo!(),
        })
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

impl std::str::FromStr for Status {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut res = Status::default();

        for line in s.lines() {
            let (key, value) = line
                .split_once(": ")
                .context(anyhow!("Invalid value '{}' when parsing Song", line))?;
            match key.to_lowercase().as_str() {
                "partition" => res.partition = value.to_owned(),
                "volume" => res.volume = Volume::new(value.parse()?),
                "repeat" => res.repeat = value != "0",
                "random" => res.random = value != "0",
                "single" => res.single = value.parse()?,
                "consume" => res.consume = value.to_owned(),
                "playlist" => res.playlist = Some(value.parse()?),
                "playlistlength" => res.playlistlength = value.parse()?,
                "state" => res.state = value.parse()?,
                "song" => res.song = Some(value.parse()?),
                "songid" => res.songid = Some(value.parse()?),
                "nextsong" => res.nextsong = Some(value.parse()?),
                "nextsongid" => res.nextsongid = Some(value.parse()?),
                "elapsed" => res.elapsed = Duration::from_secs_f32(value.parse()?),
                "duration" => res.duration = Duration::from_secs_f32(value.parse()?),
                "bitrate" => res.bitrate = Some(value.to_owned()),
                "xfade" => res.xfade = Some(value.parse()?),
                "mixrampdb" => res.mixrampdb = Some(value.to_owned()),
                "mixrampdelay" => res.mixrampdelay = Some(value.to_owned()),
                "audio" => res.audio = Some(value.to_owned()),
                "updating_db" => res.updating_db = Some(value.parse()?),
                "error" => res.error = Some(value.to_owned()),
                "time" => {} // deprecated
                key => tracing::warn!(
                    message = "Encountered unknow key/value pair while parsing 'listfiles' command",
                    key,
                    value
                ),
            }
        }
        Ok(res)
    }
}
