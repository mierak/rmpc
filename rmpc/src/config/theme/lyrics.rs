use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone)]
pub struct LyricsConfig {
    pub timestamp: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LyricsConfigFile {
    #[serde(default)]
    pub(super) timestamp: bool,
}

impl From<LyricsConfigFile> for LyricsConfig {
    fn from(value: LyricsConfigFile) -> Self {
        LyricsConfig { timestamp: value.timestamp }
    }
}
