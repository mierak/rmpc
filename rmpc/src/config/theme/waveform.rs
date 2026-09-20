use anyhow::Result;
use crossterm::style::Color;
use ratatui::{prelude::IntoCrossterm, style::Color as RatatuiColor};
use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthStr;

use super::{StyleFile, style::ToConfigOr};

#[derive(Debug, Clone)]
pub struct WaveformTheme {
    pub trace_symbol: String,
    pub trace_color: Color,
    pub show_center_line: bool,
    pub center_line_color: Color,
    pub background_color: Color,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct WaveformThemeFile {
    pub trace_symbol: String,
    pub trace_style: Option<StyleFile>,
    pub show_center_line: bool,
    pub center_line_style: Option<StyleFile>,
    pub background_color: Option<String>,
}

impl Default for WaveformThemeFile {
    fn default() -> Self {
        Self {
            trace_symbol: "▌".to_owned(),
            trace_style: Some(StyleFile { fg: Some("blue".to_owned()), bg: None, modifiers: None }),
            show_center_line: false,
            center_line_style: None,
            background_color: None,
        }
    }
}

impl WaveformThemeFile {
    pub(super) fn into_config(
        self,
        inherited_background: Option<RatatuiColor>,
    ) -> Result<WaveformTheme> {
        let background_color = super::StringColor(self.background_color)
            .to_color()?
            .or(inherited_background)
            .map_or(Color::Reset, IntoCrossterm::into_crossterm);

        let trace_style = self.trace_style.to_config_or(Some(RatatuiColor::Blue), None)?;

        anyhow::ensure!(
            UnicodeWidthStr::width(self.trace_symbol.as_str()) == 1,
            "waveform.trace_symbol must be one column wide"
        );

        let center_line_style =
            self.center_line_style.to_config_or(Some(RatatuiColor::DarkGray), None)?;

        Ok(WaveformTheme {
            trace_symbol: self.trace_symbol,
            trace_color: trace_style.fg.map_or(Color::Blue, IntoCrossterm::into_crossterm),
            show_center_line: self.show_center_line,
            center_line_color: center_line_style
                .fg
                .map_or(Color::DarkGrey, IntoCrossterm::into_crossterm),
            background_color,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::WaveformThemeFile;

    #[test]
    fn rejects_multi_column_trace_symbol() {
        let theme = WaveformThemeFile { trace_symbol: "ab".to_owned(), ..Default::default() };
        assert!(theme.into_config(None).is_err());
    }
}
