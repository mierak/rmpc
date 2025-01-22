use anyhow::{Context, anyhow};
use derive_more::{AsMut, AsRef, Into, IntoIterator};
use serde::Serialize;

use crate::mpd::errors::MpdError;
use crate::mpd::{FromMpd, LineHandled};

#[derive(Debug, Serialize, Default, IntoIterator, AsRef, AsMut, Into)]
pub struct Decoders(pub Vec<Decoder>);

#[derive(Debug, Default, Serialize)]
pub struct Decoder {
    pub name: String,
    pub suffixes: Vec<String>,
    pub mime_types: Vec<String>,
}

impl Decoder {
    fn sort(&mut self) {
        self.suffixes.sort();
        self.mime_types.sort();
    }
}

impl FromMpd for Decoders {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "plugin" {
            if let Some(last_decoder) = self.0.last_mut() {
                last_decoder.sort();
            }
            self.0.push(Decoder::default());
        }

        self.0
            .last_mut()
            .context(anyhow!(
                "No element in accumulator while parsing Decoders. Key '{}' Value :'{}'",
                key,
                value
            ))?
            .next_internal(key, value)
    }
}

impl FromMpd for Decoder {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "plugin" => self.name = value,
            "suffix" => self.suffixes.push(value),
            "mime_type" => self.mime_types.push(value),
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}
