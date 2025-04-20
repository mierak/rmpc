use std::{collections::HashMap, time::Duration};

use serde::Serialize;

use super::metadata_tag::MetadataTag;
use crate::mpd::{FromMpd, LineHandled, ParseErrorExt, errors::MpdError};

#[derive(Default, Serialize, PartialEq, Eq, Clone)]
pub struct Song {
    pub id: u32,
    pub file: String,
    pub duration: Option<Duration>,
    pub metadata: HashMap<String, MetadataTag>,
    pub stickers: Option<HashMap<String, String>>,
}

impl std::fmt::Debug for Song {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Song {{ file: {}, title: {:?}, artist: {:?}, id: {}, track: {:?} }}",
            self.file,
            self.metadata.get("title"),
            self.metadata.get("artist"),
            self.id,
            self.metadata.get("track")
        )
    }
}

impl FromMpd for Song {
    fn next_internal(&mut self, key: &str, mut value: String) -> Result<LineHandled, MpdError> {
        match key {
            "file" => self.file = value,
            "id" => self.id = value.parse().logerr(key, &value)?,
            "duration" => {
                self.duration = Some(Duration::from_secs_f64(value.parse().logerr(key, &value)?));
            }
            "time" | "format" => {} // deprecated or ignored
            key => {
                self.metadata
                    .entry(key.to_owned())
                    .and_modify(|present| match present {
                        MetadataTag::Single(current) => {
                            *present = MetadataTag::Multiple(vec![
                                std::mem::take(current),
                                std::mem::take(&mut value),
                            ]);
                        }
                        MetadataTag::Multiple(items) => {
                            items.push(std::mem::take(&mut value));
                        }
                    })
                    .or_insert(MetadataTag::Single(value));
            }
        }
        Ok(LineHandled::Yes)
    }
}
