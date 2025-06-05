use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone)]
pub struct LyricsConfig {
    pub show_timestamp: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricsConfigFile {
    pub(super) show_timestamp: bool,
}

impl Default for LyricsConfigFile {
    fn default() -> Self {
        Self { show_timestamp: false }
    }
}

impl LyricsConfigFile {
    pub(super) fn into_config(self) -> Result<LyricsConfig> {
        Ok(LyricsConfig { show_timestamp: self.show_timestamp })
    }
}
