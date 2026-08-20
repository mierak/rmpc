use std::{collections::HashMap, time::Duration};

use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::Serialize;

use super::metadata_tag::MetadataTag;
use crate::{
    errors::MpdError,
    from_mpd::{FromMpd, LineHandled, ParseErrorExt},
};

#[derive(Default, Serialize, PartialEq, Eq, Clone)]
pub struct Song {
    pub id: u32,
    pub file: String,
    pub duration: Option<Duration>,
    pub metadata: HashMap<String, MetadataTag>,
    pub last_modified: DateTime<Utc>,
    // Option because it is present from mpd 0.24 onwards
    pub added: Option<DateTime<Utc>>,
}

impl Song {
    pub fn samplerate(&self) -> Option<u32> {
        self.metadata.get("format").and_then(|audio| super::parse_audio_format(audio.first()).0)
    }

    pub fn bits(&self) -> Option<u32> {
        self.metadata.get("format").and_then(|audio| super::parse_audio_format(audio.first()).1)
    }

    pub fn channels(&self) -> Option<u32> {
        self.metadata.get("format").and_then(|audio| super::parse_audio_format(audio.first()).2)
    }
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
            "time" => {} // deprecated or ignored
            "last-modified" => {
                self.last_modified =
                    value.parse().context("Failed to parse date").logerr(key, &value)?;
            }
            "added" => {
                self.added =
                    Some(value.parse().context("Failed to parse date").logerr(key, &value)?);
            }
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rstest::rstest;

    use super::Song;
    use crate::commands::metadata_tag::MetadataTag;

    // Same grammar as the status "audio" field; songs carry it in the
    // "Format" metadata key. See the table in status.rs for the source of
    // the strings.
    #[rstest]
    #[case("44100:16:2", Some(44100), Some(16), Some(2))]
    #[case("44100:f:2", Some(44100), None, Some(2))]
    #[case("dsd64:2", Some(2_822_400), Some(1), Some(2))]
    #[case("384000:dsd:2", Some(3_072_000), Some(1), Some(2))]
    #[case("*:*:*", None, None, None)]
    fn format_metadata_parses(
        #[case] format: &str,
        #[case] samplerate: Option<u32>,
        #[case] bits: Option<u32>,
        #[case] channels: Option<u32>,
    ) {
        let mut song = Song::default();
        song.metadata.insert("format".to_owned(), MetadataTag::Single(format.to_owned()));

        assert_eq!(song.samplerate(), samplerate);
        assert_eq!(song.bits(), bits);
        assert_eq!(song.channels(), channels);
    }

    #[test]
    fn format_metadata_absent() {
        let song = Song::default();

        assert_eq!(song.samplerate(), None);
        assert_eq!(song.bits(), None);
        assert_eq!(song.channels(), None);
    }
}
