use anyhow::Result;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};

use super::{StyleFile, style::ToConfigOr};

#[derive(Debug, Default, Clone)]
pub struct LyricsConfig {
    pub active_line_style: Style,
    pub line_style: Style,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricsConfigFile {
    pub(super) active_line_style: StyleFile,
    pub(super) line_style: StyleFile,
}

impl Default for LyricsConfigFile {
    fn default() -> Self {
        Self { active_line_style: StyleFile::default(), line_style: StyleFile::default() }
    }
}

impl LyricsConfigFile {
    pub(super) fn into_config(self) -> Result<LyricsConfig> {
        Ok(LyricsConfig {
            active_line_style: self.active_line_style.to_config_or(Some(Color::Blue), None)?,
            line_style: self.line_style.to_config_or(Some(Color::White), None)?,
        })
    }
}
