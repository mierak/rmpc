use anyhow::{anyhow, Context};
use derive_more::{AsMut, AsRef, Into, IntoIterator};
use serde::Serialize;

use crate::mpd::{errors::MpdError, FromMpd, LineHandled};

#[derive(Debug, Default, Serialize, IntoIterator, AsRef, AsMut, Into)]
pub struct Stickers(pub Vec<Sticker>);

#[derive(Debug, Default, Serialize, IntoIterator, AsRef, AsMut, Into)]
pub struct StickersWithFile(pub Vec<StickerWithFile>);

#[derive(Debug, Serialize, Default)]
pub struct Sticker {
    key: String,
    value: String,
}

#[derive(Debug, Serialize, Default)]
pub struct StickerWithFile {
    file: String,
    key: String,
    value: String,
}

impl From<Vec<Sticker>> for Stickers {
    fn from(value: Vec<Sticker>) -> Self {
        Stickers(value)
    }
}

impl From<Vec<StickerWithFile>> for StickersWithFile {
    fn from(value: Vec<StickerWithFile>) -> Self {
        StickersWithFile(value)
    }
}

impl FromMpd for Stickers {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        let mut sticker = Sticker::default();
        Sticker::next_internal(&mut sticker, key, value)?;

        self.0.push(sticker);
        Ok(LineHandled::Yes)
    }
}

impl FromMpd for Sticker {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "sticker" => {
                let Some((name, value)) = value.split_once('=') else {
                    return Err(MpdError::Parse(format!("Invalid sticker value: {value}")));
                };
                name.clone_into(&mut self.key);
                value.clone_into(&mut self.value);
            }
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}

impl FromMpd for StickersWithFile {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "file" {
            self.0.push(StickerWithFile::default());
        }

        self.0
            .last_mut()
            .context(anyhow!(
                "No element in accumulator while parsing StickersWithFile. Key '{}' Value :'{}'",
                key,
                value
            ))?
            .next_internal(key, value)
    }
}

impl FromMpd for StickerWithFile {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        match key {
            "file" => {
                self.file = value;
            }
            "sticker" => {
                let Some((name, value)) = value.split_once('=') else {
                    return Err(MpdError::Parse(format!("Invalid sticker value: {value}")));
                };
                name.clone_into(&mut self.key);
                value.clone_into(&mut self.value);
            }
            _ => return Ok(LineHandled::No { value }),
        }
        Ok(LineHandled::Yes)
    }
}
