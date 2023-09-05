use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

use super::Song;
use anyhow::Context;

#[derive(Debug, Default)]
pub struct Songs(pub Vec<Song>);

impl FromMpd for Songs {
    fn finish(self) -> std::result::Result<Self, crate::mpd::errors::MpdError> {
        Ok(self)
    }

    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "file" {
            self.0.push(Song::default());
        }
        self.0
            .last_mut()
            .context("No element in accumulator while parsing PlayListInfo")?
            .next_internal(key, value)
    }
}
