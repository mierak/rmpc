use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone)]
pub struct LyricsConfig {
    pub show_timestamp: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct LyricsConfigFile {
    pub(super) show_timestamp: bool,
}

impl TryFrom<LyricsConfigFile> for LyricsConfig {
    type Error = anyhow::Error;

    fn try_from(value: LyricsConfigFile) -> Result<Self> {
        Ok(LyricsConfig { show_timestamp: value.show_timestamp })
    }
}
