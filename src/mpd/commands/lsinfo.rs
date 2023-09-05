use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

use super::Song;
use anyhow::Context;

#[derive(Debug, Default)]
pub struct LsInfo(pub Vec<FileOrDir>);

#[derive(Debug, PartialEq, Eq)]
pub enum FileOrDir {
    Dir(Dir),
    File(Song),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Dir {
    /// this is the full path from mpd root
    pub path: String,
    pub full_path: String,
    pub last_modified: String,
}

impl FromMpd for Dir {
    fn finish(self) -> std::result::Result<Self, crate::mpd::errors::MpdError> {
        Ok(self)
    }

    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "directory" => {
                self.path = value.split('/').last().context("Failed to parse dir name.")?.to_owned();
                self.full_path = value;
            }
            "last-modified" => self.last_modified = value,
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}

impl FromMpd for LsInfo {
    fn finish(self) -> std::result::Result<Self, crate::mpd::errors::MpdError> {
        Ok(self)
    }

    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "file" {
            self.0.push(FileOrDir::File(Song::default()));
        }
        if key == "directory" {
            self.0.push(FileOrDir::Dir(Dir::default()));
        }

        match self
            .0
            .last_mut()
            .context("No element in accumulator while parsing LsInfo")?
        {
            FileOrDir::Dir(dir) => dir.next_internal(key, value),
            FileOrDir::File(song) => song.next_internal(key, value),
        }
    }
}
