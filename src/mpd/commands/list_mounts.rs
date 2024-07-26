use anyhow::Context;
use derive_more::{AsMut, AsRef, Into, IntoIterator};
use serde::Serialize;

use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

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
            .context("No element in accumulator while parsing Mounts")?
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
