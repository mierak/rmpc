use serde::Serialize;

use crate::{
    errors::MpdError,
    from_mpd::{FromMpd, LineHandled},
};

#[derive(Debug, Default, Serialize, PartialEq, Eq, Clone)]
pub struct MpdConfig {
    pub music_directory: String,
    pub playlist_directory: String,
    pub pcre: bool,
}

impl FromMpd for MpdConfig {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "music_directory" => self.music_directory = value,
            "playlist_directory" => self.playlist_directory = value,
            "pcre" if value == "1" => self.pcre = true,
            "pcre" if value == "0" => self.pcre = false,
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}
