use ::serde::{Deserialize, Serialize};
use ratatui::style::Style;

use super::{StyleFile, ToConfigOr};

#[derive(derive_more::Debug, Default, Clone)]
pub struct LevelStyles {
    pub trace: Style,
    pub debug: Style,
    pub warn: Style,
    pub error: Style,
    pub info: Style,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LevelStylesFile {
    trace: StyleFile,
    debug: StyleFile,
    warn: StyleFile,
    error: StyleFile,
    info: StyleFile,
}

impl Default for LevelStylesFile {
    fn default() -> Self {
        Self {
            trace: StyleFile {
                fg: Some("magenta".to_string()),
                bg: Some("black".to_string()),
                modifiers: None,
            },
            debug: StyleFile {
                fg: Some("light_green".to_string()),
                bg: Some("black".to_string()),
                modifiers: None,
            },
            warn: StyleFile {
                fg: Some("yellow".to_string()),
                bg: Some("black".to_string()),
                modifiers: None,
            },
            error: StyleFile {
                fg: Some("red".to_string()),
                bg: Some("black".to_string()),
                modifiers: None,
            },
            info: StyleFile {
                fg: Some("blue".to_string()),
                bg: Some("black".to_string()),
                modifiers: None,
            },
        }
    }
}

impl TryFrom<LevelStylesFile> for LevelStyles {
    type Error = anyhow::Error;

    fn try_from(value: LevelStylesFile) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            trace: value.trace.to_config_or(None, None)?,
            debug: value.debug.to_config_or(None, None)?,
            warn: value.warn.to_config_or(None, None)?,
            error: value.error.to_config_or(None, None)?,
            info: value.info.to_config_or(None, None)?,
        })
    }
}
