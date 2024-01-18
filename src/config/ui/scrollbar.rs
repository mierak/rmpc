use anyhow::Result;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use super::{color::FgBgColorsExt, FgBgColors, FgBgColorsFile};

#[derive(Debug)]
pub struct ScrollbarConfig {
    /// Symbols used for the scrollbar
    /// First symbol is used for the scrollbar track
    /// Second symbol is used for the scrollbar thumb
    /// Third symbol is used for the scrollbar up button
    /// Fourth symbol is used for the scrollbar down button
    pub symbols: [&'static str; 4],
    /// Fall sback to border color for foreground and default color for background
    pub track_colors: FgBgColors,
    /// Fall sback to border color for foreground and default color for background
    pub ends_colors: FgBgColors,
    // Falls back to blue for foreground and default color for background
    pub thumb_colors: FgBgColors,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScrollbarConfigFile {
    pub(super) symbols: Vec<String>,
    pub(super) track_colors: Option<FgBgColorsFile>,
    pub(super) ends_colors: Option<FgBgColorsFile>,
    pub(super) thumb_colors: Option<FgBgColorsFile>,
}

impl Default for ScrollbarConfigFile {
    fn default() -> Self {
        Self {
            symbols: vec!["║".to_owned(), "█".to_owned(), "▲".to_owned(), "▼".to_owned()],
            track_colors: Some(FgBgColorsFile { fg: None, bg: None }),
            ends_colors: Some(FgBgColorsFile { fg: None, bg: None }),
            thumb_colors: Some(FgBgColorsFile {
                fg: Some("blue".to_string()),
                bg: None,
            }),
        }
    }
}

impl ScrollbarConfigFile {
    pub(super) fn into_config(mut self, fallback_color: Color) -> Result<ScrollbarConfig> {
        let sb_track = std::mem::take(&mut self.symbols[0]);
        let sb_thumb = std::mem::take(&mut self.symbols[1]);
        let sb_up = std::mem::take(&mut self.symbols[2]);
        let sb_down = std::mem::take(&mut self.symbols[3]);

        Ok(ScrollbarConfig {
            symbols: [
                Box::leak(Box::new(sb_track)),
                Box::leak(Box::new(sb_thumb)),
                Box::leak(Box::new(sb_up)),
                Box::leak(Box::new(sb_down)),
            ],
            ends_colors: self.ends_colors.to_config_or(fallback_color, Color::Reset)?,
            thumb_colors: self.thumb_colors.to_config_or(Color::Blue, Color::Reset)?,
            track_colors: self.track_colors.to_config_or(fallback_color, Color::Reset)?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::needless_pass_by_value)]
mod tests {
    use crate::config::ui::{scrollbar::ScrollbarConfigFile, FgBgColors, FgBgColorsFile};
    use ratatui::style::Color as RC;
    use test_case::test_case;

    #[test]
    fn maps_symbols() {
        let input = ScrollbarConfigFile {
            symbols: vec!["a".to_owned(), "b".to_owned(), "c".to_owned(), "d".to_owned()],
            ..Default::default()
        };

        let result = input.into_config(RC::Red).unwrap().symbols;

        assert_eq!(result, ["a".to_owned(), "b".to_owned(), "c".to_owned(), "d".to_owned()]);
    }

    #[test_case(None,         None,         FgBgColors { fg: RC::Blue, bg: RC::Reset }  ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), FgBgColors { fg: RC::Blue, bg: RC::Reset }  ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), FgBgColors { fg: RC::Red,  bg: RC::Blue }   ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         FgBgColors { fg: RC::Cyan, bg: RC::Reset }  ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), FgBgColors { fg: RC::Blue, bg: RC::Gray }   ; "correctly maps when only bg is provided")]
    fn thumb_colors_test(c1: Option<&str>, c2: Option<&str>, expected: FgBgColors) {
        let fallback = RC::DarkGray;
        let input = ScrollbarConfigFile {
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

        let result = input.into_config(fallback).unwrap();

        assert_eq!(result.thumb_colors, expected);
    }

    #[test_case(None,         None,         FgBgColors { fg: RC::DarkGray, bg: RC::Reset }  ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), FgBgColors { fg: RC::DarkGray, bg: RC::Reset }  ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), FgBgColors { fg: RC::Red,      bg: RC::Blue }   ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         FgBgColors { fg: RC::Cyan,     bg: RC::Reset }  ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), FgBgColors { fg: RC::DarkGray, bg: RC::Gray }   ; "correctly maps when only bg is provided")]
    fn ends_colors_test(c1: Option<&str>, c2: Option<&str>, expected: FgBgColors) {
        let fallback = RC::DarkGray;
        let input = ScrollbarConfigFile {
            ends_colors: match (c1, c2) {
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

        let result = input.into_config(fallback).unwrap();

        assert_eq!(result.ends_colors, expected);
    }

    #[test_case(None,         None,         FgBgColors { fg: RC::DarkGray, bg: RC::Reset }  ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), FgBgColors { fg: RC::DarkGray, bg: RC::Reset }  ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), FgBgColors { fg: RC::Red,      bg: RC::Blue }   ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         FgBgColors { fg: RC::Cyan,     bg: RC::Reset }  ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), FgBgColors { fg: RC::DarkGray, bg: RC::Gray }   ; "correctly maps when only bg is provided")]
    fn track_colors_test(c1: Option<&str>, c2: Option<&str>, expected: FgBgColors) {
        let fallback = RC::DarkGray;
        let input = ScrollbarConfigFile {
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

        let result = input.into_config(fallback).unwrap();

        assert_eq!(result.track_colors, expected);
    }
}
