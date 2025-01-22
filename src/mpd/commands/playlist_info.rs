use anyhow::{Context, anyhow};

use super::Song;
use crate::mpd::errors::MpdError;
use crate::mpd::{FromMpd, LineHandled};

impl FromMpd for Vec<Song> {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "file" {
            self.push(Song::default());
        }
        self.last_mut()
            .context(anyhow!(
                "No element in accumulator while parsing PlayListInfo. Key '{}' Value :'{}'",
                key,
                value
            ))?
            .next_internal(key, value)
    }
}
