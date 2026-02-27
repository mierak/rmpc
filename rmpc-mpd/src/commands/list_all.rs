use derive_more::{AsMut, AsRef, Into, IntoIterator};

use crate::{
    errors::MpdError,
    from_mpd::{FromMpd, LineHandled},
};

#[derive(Debug, Default, IntoIterator, AsRef, AsMut, Into)]
pub struct ListAll(pub Vec<ListAllEntry>);

impl ListAll {
    pub fn into_files(self) -> impl Iterator<Item = String> {
        self.into_iter().filter_map(|item| match item {
            ListAllEntry::File(file) => Some(file),
            ListAllEntry::Dir(_) => None,
            ListAllEntry::Playlist(_) => None,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ListAllEntry {
    Dir(String),
    File(String),
    Playlist(String),
}

impl FromMpd for ListAll {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "file" => {
                self.0.push(ListAllEntry::File(value));
                Ok(LineHandled::Yes)
            }
            "directory" => {
                self.0.push(ListAllEntry::Dir(value));
                Ok(LineHandled::Yes)
            }
            "playlist" => {
                self.0.push(ListAllEntry::Playlist(value));
                Ok(LineHandled::Yes)
            }
            _ => Ok(LineHandled::No { value }),
        }
    }
}
