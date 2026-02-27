use derive_more::{AsMut, AsRef, Into, IntoIterator};

use crate::{
    errors::MpdError,
    from_mpd::{FromMpd, LineHandled},
};

#[derive(Debug, Default, IntoIterator, AsRef, AsMut, Into)]
pub struct FileList(pub Vec<String>);

impl FromMpd for FileList {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "file" => {
                self.0.push(value);
                Ok(LineHandled::Yes)
            }
            _ => Ok(LineHandled::No { value }),
        }
    }
}
