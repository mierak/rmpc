use derive_more::{AsMut, AsRef, Into, IntoIterator};

use crate::mpd::errors::MpdError;
use crate::mpd::{FromMpd, LineHandled};

#[derive(Debug, Default, IntoIterator, AsRef, AsMut, Into)]
pub struct MpdList(pub Vec<String>);

impl From<Vec<String>> for MpdList {
    fn from(value: Vec<String>) -> Self {
        MpdList(value)
    }
}

impl FromMpd for MpdList {
    fn next_internal(&mut self, _key: &str, value: String) -> Result<LineHandled, MpdError> {
        self.0.push(value);
        Ok(LineHandled::Yes)
    }
}
