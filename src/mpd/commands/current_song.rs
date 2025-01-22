use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;

use crate::mpd::errors::MpdError;
use crate::mpd::{FromMpd, LineHandled, ParseErrorExt};

#[derive(Default, Serialize, PartialEq, Eq, Clone)]
pub struct Song {
    pub id: u32,
    pub file: String,
    pub duration: Option<Duration>,
    pub metadata: HashMap<String, String>,
    pub stickers: Option<HashMap<String, String>>,
}

impl std::fmt::Debug for Song {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Song {{ file: {}, title: {:?}, artist: {:?}, id: {}, track: {:?} }}",
            self.file,
            self.title(),
            self.artist(),
            self.id,
            self.metadata.get("track")
        )
    }
}

impl Song {
    pub fn title(&self) -> Option<&String> {
        self.metadata.get("title")
    }

    pub fn artist(&self) -> Option<&String> {
        self.metadata.get("artist")
    }

    pub fn album(&self) -> Option<&String> {
        self.metadata.get("album")
    }
}

impl FromMpd for Song {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "file" => self.file = value,
            "id" => self.id = value.parse().logerr(key, &value)?,
            "duration" => {
                self.duration = Some(Duration::from_secs_f64(value.parse().logerr(key, &value)?));
            }
            "time" | "format" => {} // deprecated or ignored
            key => {
                self.metadata.insert(key.to_owned(), value);
            }
        }
        Ok(LineHandled::Yes)
    }
}
