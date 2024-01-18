use anyhow::Result;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use super::{color::FgBgColorsExt, FgBgColors, FgBgColorsFile};

#[derive(Debug)]
pub struct ProgressBarConfig {
    /// Symbols for the rogress bar at the bottom of the screen
    /// First symbol is used for the elapsed part of the progress bar
    /// Second symbol is used for the thumb
    /// Third symbol is used for the remaining part of the progress bar
    pub symbols: [&'static str; 3],
    /// Fall sback to black for foreground and default color for background
    /// For transparent track you should set the track symbol to empty string
    pub track_colors: FgBgColors,
    /// Fall sback to blue for foreground and black for background
    pub elapsed_colors: FgBgColors,
    /// Thumb at the end of the elapsed part of the progress bar
    /// Fall sback to blue for foreground and black for background
    pub thumb_colors: FgBgColors,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProgressBarConfigFile {
    pub(super) symbols: Vec<String>,
    pub(super) track_colors: Option<FgBgColorsFile>,
    pub(super) elapsed_colors: Option<FgBgColorsFile>,
    pub(super) thumb_colors: Option<FgBgColorsFile>,
}

impl Default for ProgressBarConfigFile {
    fn default() -> Self {
        Self {
            symbols: vec!["█".to_owned(), "".to_owned(), "█".to_owned()],
            track_colors: Some(FgBgColorsFile {
                fg: Some("black".to_string()),
                bg: Some("default".to_string()),
            }),
            elapsed_colors: Some(FgBgColorsFile {
                fg: Some("blue".to_string()),
                bg: Some("default".to_string()),
            }),
            thumb_colors: Some(FgBgColorsFile {
                fg: Some("blue".to_string()),
                bg: Some("black".to_string()),
            }),
        }
    }
}

impl ProgressBarConfigFile {
    pub(super) fn into_config(mut self) -> Result<ProgressBarConfig> {
        let elapsed = std::mem::take(&mut self.symbols[0]);
        let thumb = std::mem::take(&mut self.symbols[1]);
        let track = std::mem::take(&mut self.symbols[2]);

        Ok(ProgressBarConfig {
            symbols: [
                Box::leak(Box::new(elapsed)),
                Box::leak(Box::new(thumb)),
                Box::leak(Box::new(track)),
            ],
            elapsed_colors: self.elapsed_colors.to_config_or(Color::Blue, Color::Black)?,
            thumb_colors: self.thumb_colors.to_config_or(Color::Blue, Color::Black)?,
            track_colors: self.track_colors.to_config_or(Color::Black, Color::Black)?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::needless_pass_by_value)]
mod tests {
    use crate::config::ui::{progress_bar::ProgressBarConfigFile, FgBgColors, FgBgColorsFile};
    use ratatui::style::Color as RC;
    use test_case::test_case;

    #[test]
    fn maps_symbols() {
        let input = ProgressBarConfigFile {
            symbols: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
            ..Default::default()
        };

        let result = input.into_config().unwrap().symbols;

        assert_eq!(result, ["a".to_owned(), "b".to_owned(), "c".to_owned()]);
    }

    #[test_case(None,         None,         FgBgColors { fg: RC::Blue, bg: RC::Black }  ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), FgBgColors { fg: RC::Blue, bg: RC::Black }  ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), FgBgColors { fg: RC::Red,  bg: RC::Blue }   ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         FgBgColors { fg: RC::Cyan, bg: RC::Black }  ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), FgBgColors { fg: RC::Blue, bg: RC::Gray }   ; "correctly maps when only bg is provided")]
    fn elapsed_colors_test(c1: Option<&str>, c2: Option<&str>, expected: FgBgColors) {
        let input = ProgressBarConfigFile {
            elapsed_colors: match (c1, c2) {
                (Some("none"), Some("none")) => None,
                (Some(c1), Some(c2)) => Some(FgBgColorsFile {
                    fg: Some(c1.to_string()),
                    bg: Some(c2.to_string()),
                }),
                (Some(c1), None) => Some(FgBgColorsFile {
                    fg: Some(c1.to_string()),
                    bg: None,
                }),
                (None, Some(c2)) => Some(FgBgColorsFile {
                    fg: None,
                    bg: Some(c2.to_string()),
                }),
                (None, None) => Some(FgBgColorsFile { fg: None, bg: None }),
            },
            ..Default::default()
        };

        let result = input.into_config().unwrap();

        assert_eq!(result.elapsed_colors, expected);
    }

    #[test_case(None,         None,         FgBgColors { fg: RC::Black, bg: RC::Black }  ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), FgBgColors { fg: RC::Black, bg: RC::Black }  ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), FgBgColors { fg: RC::Red,   bg: RC::Blue }   ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         FgBgColors { fg: RC::Cyan,  bg: RC::Black }  ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), FgBgColors { fg: RC::Black, bg: RC::Gray }   ; "correctly maps when only bg is provided")]
    fn track_colors_test(c1: Option<&str>, c2: Option<&str>, expected: FgBgColors) {
        let input = ProgressBarConfigFile {
            track_colors: match (c1, c2) {
                (Some("none"), Some("none")) => None,
                (Some(c1), Some(c2)) => Some(FgBgColorsFile {
                    fg: Some(c1.to_string()),
                    bg: Some(c2.to_string()),
                }),
                (Some(c1), None) => Some(FgBgColorsFile {
                    fg: Some(c1.to_string()),
                    bg: None,
                }),
                (None, Some(c2)) => Some(FgBgColorsFile {
                    fg: None,
                    bg: Some(c2.to_string()),
                }),
                (None, None) => Some(FgBgColorsFile { fg: None, bg: None }),
            },
            ..Default::default()
        };

        let result = input.into_config().unwrap();

        assert_eq!(result.track_colors, expected);
    }

    #[test_case(None,         None,         FgBgColors { fg: RC::Blue, bg: RC::Black }  ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), FgBgColors { fg: RC::Blue, bg: RC::Black }  ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), FgBgColors { fg: RC::Red,  bg: RC::Blue }   ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         FgBgColors { fg: RC::Cyan, bg: RC::Black }  ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), FgBgColors { fg: RC::Blue, bg: RC::Gray }   ; "correctly maps when only bg is provided")]
    fn thumb_colors_test(c1: Option<&str>, c2: Option<&str>, expected: FgBgColors) {
        let input = ProgressBarConfigFile {
            thumb_colors: match (c1, c2) {
                (Some("none"), Some("none")) => None,
                (Some(c1), Some(c2)) => Some(FgBgColorsFile {
                    fg: Some(c1.to_string()),
                    bg: Some(c2.to_string()),
                }),
                (Some(c1), None) => Some(FgBgColorsFile {
                    fg: Some(c1.to_string()),
                    bg: None,
                }),
                (None, Some(c2)) => Some(FgBgColorsFile {
                    fg: None,
                    bg: Some(c2.to_string()),
                }),
                (None, None) => Some(FgBgColorsFile { fg: None, bg: None }),
            },
            ..Default::default()
        };

        let result = input.into_config().unwrap();

        assert_eq!(result.thumb_colors, expected);
    }
}
