use anyhow::{Context, anyhow};

use super::Song;
use crate::mpd::{FromMpd, LineHandled, errors::MpdError};

impl FromMpd for Vec<Song> {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "file" {
            self.push(Song::default());
        }
        self.last_mut()
            .context(anyhow!(
                "No element in accumulator while parsing PlayListInfo. Key '{key}' Value :'{value}'"
            ))?
            .next_internal(key, value)
    }
}
