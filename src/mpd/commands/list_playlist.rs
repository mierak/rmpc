use derive_more::{AsMut, AsRef, Into, IntoIterator};

use crate::mpd::{FromMpd, LineHandled};

#[derive(Debug, Default, IntoIterator, AsRef, AsMut, Into)]
pub struct FileList(Vec<String>);

impl FromMpd for FileList {
    fn next_internal(
        &mut self,
        key: &str,
        value: String,
    ) -> Result<crate::mpd::LineHandled, crate::mpd::errors::MpdError> {
        match key {
            "file" => {
                self.0.push(value);
                Ok(LineHandled::Yes)
            }
            _ => Ok(LineHandled::No { value }),
        }
    }
}
