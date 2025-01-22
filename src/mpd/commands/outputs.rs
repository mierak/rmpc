use anyhow::{Context, anyhow};
use derive_more::{AsMut, AsRef, Into, IntoIterator};
use serde::Serialize;

use crate::mpd::{FromMpd, LineHandled, ParseErrorExt, errors::MpdError};

#[derive(Debug, Serialize, Default, IntoIterator, AsRef, AsMut, Into)]
pub struct Outputs(pub Vec<Output>);

#[derive(Debug, Default, Serialize)]
pub struct Output {
    pub id: u32,
    pub name: String,
    pub enabled: bool,
}

impl FromMpd for Outputs {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "outputid" {
            self.0.push(Output::default());
        }

        self.0
            .last_mut()
            .context(anyhow!(
                "No element in accumulator while parsing Outputs. Key '{}' Value :'{}'",
                key,
                value
            ))?
            .next_internal(key, value)
    }
}

impl FromMpd for Output {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "outputid" => self.id = value.parse().logerr(key, &value)?,
            "outputname" => self.name = value,
            "outputenabled" => match value.as_str() {
                "0" => self.enabled = false,
                "1" => self.enabled = true,
                _ => return Ok(LineHandled::No { value }),
            },
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}
