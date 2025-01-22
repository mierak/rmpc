use anyhow::{Context, anyhow};

use crate::mpd::errors::MpdError;
use crate::mpd::{FromMpd, LineHandled};

#[derive(Default, Debug)]
pub struct Playlist {
    pub name: String,
    pub last_modified: String,
}

impl FromMpd for Playlist {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "playlist" => {
                self.name = value;
            }
            "last-modified" => self.last_modified = value,
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}

impl FromMpd for Vec<Playlist> {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "playlist" {
            self.push(Playlist::default());
        }

        self.last_mut()
            .context(anyhow!(
                "No element in accumulator while parsing Playlists. Key '{}' Value :'{}'",
                key,
                value
            ))?
            .next_internal(key, value)
    }
}
