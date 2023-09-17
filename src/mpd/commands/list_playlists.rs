use anyhow::Context;

use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

#[derive(Default, Debug)]
pub struct Playlist {
    pub name: String,
    pub last_modified: String, // TODO timestamp?
}

impl FromMpd for Playlist {
    fn finish(self) -> Result<Self, crate::mpd::errors::MpdError> {
        Ok(self)
    }

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
    fn finish(self) -> Result<Self, MpdError> {
        Ok(self)
    }

    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "playlist" {
            self.push(Playlist::default());
        }

        self.last_mut()
            .context("No element in accumulator while parsing Playlists")?
            .next_internal(key, value)
    }
}
