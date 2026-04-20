use serde::{Deserialize, Serialize};

use crate::config::theme::properties::Alignment;

#[derive(Debug, Default, Clone)]
pub struct LyricsConfig {
    pub timestamp: bool,
    pub alignment: Alignment,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LyricsConfigFile {
    pub(super) timestamp: bool,
    pub(super) alignment: Alignment,
}

impl Default for LyricsConfigFile {
    fn default() -> Self {
        Self { timestamp: false, alignment: Alignment::Center }
    }
}

impl From<LyricsConfigFile> for LyricsConfig {
    fn from(value: LyricsConfigFile) -> Self {
        LyricsConfig { timestamp: value.timestamp, alignment: value.alignment }
    }
}
