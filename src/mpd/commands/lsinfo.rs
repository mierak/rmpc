use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

use super::Song;
use anyhow::anyhow;
use anyhow::Context;
use derive_more::{AsMut, AsRef, Into, IntoIterator};

#[derive(Debug, Default, IntoIterator, AsRef, AsMut, Into)]
pub struct LsInfo(pub Vec<FileOrDir>);

#[derive(Debug, PartialEq, Eq)]
pub enum FileOrDir {
    Dir(Dir),
    File(Song),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Dir {
    /// Last segment of the part, the dir name
    pub path: String,
    /// this is the full path from mpd root
    pub full_path: String,
    pub last_modified: String,
}

impl FromMpd for Dir {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "directory" => {
                value
                    .split('/')
                    .last()
                    .context(anyhow!("Failed to parse dir name. Key: '{}' Value: '{}'", key, value))?
                    .clone_into(&mut self.path);
                self.full_path = value;
            }
            "last-modified" => self.last_modified = value,
            "playlist" => {} // ignore, deprecated
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}

impl FromMpd for LsInfo {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "file" {
            self.0.push(FileOrDir::File(Song::default()));
        }
        if key == "directory" {
            self.0.push(FileOrDir::Dir(Dir::default()));
        }

        match self.0.last_mut().context(anyhow!(
            "No element in accumulator while parsing LsInfo. Key '{}' Value :'{}'",
            key,
            value
        ))? {
            FileOrDir::Dir(dir) => dir.next_internal(key, value),
            FileOrDir::File(song) => song.next_internal(key, value),
        }
    }
}
