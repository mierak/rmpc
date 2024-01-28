use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

use super::Song;
use anyhow::Context;

impl FromMpd for Vec<Song> {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "file" {
            self.push(Song::default());
        }
        self.last_mut()
            .context("No element in accumulator while parsing PlayListInfo")?
            .next_internal(key, value)
    }
}
