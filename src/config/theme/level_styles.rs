use ::serde::{Deserialize, Serialize};
use ratatui::style::Style;

use super::{StyleFile, ToConfigOr, defaults};

#[derive(derive_more::Debug, Default, Clone)]
pub struct LevelStyles {
    pub trace: Style,
    pub debug: Style,
    pub warn: Style,
    pub error: Style,
    pub info: Style,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LevelStylesFile {
    #[serde(default = "defaults::default_trace_color")]
    trace: StyleFile,
    #[serde(default = "defaults::default_debug_color")]
    debug: StyleFile,
    #[serde(default = "defaults::default_warn_color")]
    warn: StyleFile,
    #[serde(default = "defaults::default_error_color")]
    error: StyleFile,
    #[serde(default = "defaults::default_info_color")]
    info: StyleFile,
}

impl Default for LevelStylesFile {
    fn default() -> Self {
        Self {
            trace: defaults::default_trace_color(),
            debug: defaults::default_debug_color(),
            warn: defaults::default_warn_color(),
            error: defaults::default_error_color(),
            info: defaults::default_info_color(),
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
