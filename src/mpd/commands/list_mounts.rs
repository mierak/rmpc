use anyhow::{Context, anyhow};
use derive_more::{AsMut, AsRef, Into, IntoIterator};
use serde::Serialize;

use crate::mpd::{FromMpd, LineHandled, errors::MpdError};

#[derive(Debug, Serialize, Default, IntoIterator, AsRef, AsMut, Into)]
pub struct Mounts(pub Vec<Mount>);

#[derive(Debug, Default, Serialize)]
pub struct Mount {
    pub mount: String,
    pub storage: String,
}

impl FromMpd for Mounts {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "mount" {
            self.0.push(Mount::default());
        }

        self.0
            .last_mut()
            .context(anyhow!(
                "No element in accumulator while parsing Mounts. Key '{}' Value :'{}'",
                key,
                value
            ))?
            .next_internal(key, value)
    }
}

impl FromMpd for Mount {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "mount" => self.mount = value,
            "storage" => self.storage = value,
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}
