use anyhow::Context;
use derive_more::{AsMut, AsRef, Into, IntoIterator};

use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

// file: 03 Diode.flac
// size: 18183774
// Last-Modified: 2022-12-24T13:02:09Z
#[derive(Debug, Default, IntoIterator, AsRef, AsMut, Into)]
pub struct ListFiles(Vec<Listed>);
#[derive(Debug, Default)]
pub struct Listed {
    pub kind: ListingType,
    pub name: String,
    pub size: u64,
    pub last_modified: String, // TODO timestamp?
}

#[derive(Debug, Default)]
pub enum ListingType {
    #[default]
    File,
    Dir,
}

impl FromMpd for ListFiles {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "file" || key == "directory" {
            self.0.push(Listed::default());
        }

        self.0
            .last_mut()
            .context("No element in accumulator while parsing ListFiles")?
            .next_internal(key, value)
    }
}

impl FromMpd for Listed {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "file" => {
                self.kind = ListingType::File;
                self.name = value;
            }
            "directory" => {
                self.kind = ListingType::Dir;
                self.name = value;
            }
            "size" => self.size = value.parse()?,
            "last-modified" => self.last_modified = value,
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}
