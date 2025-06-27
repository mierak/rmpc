use anyhow::Result;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};

use super::{StyleFile, style::ToConfigOr};

#[derive(Debug, Default, Clone, Hash, Eq, PartialEq)]
pub struct VolumeSliderConfig {
    /// Symbols for the volume slider
    /// First symbol is used for the start boundary of the volume slider
    /// Second symbol is used for the filled part of the volume slider
    /// Third symbol is used for the thumb
    /// Fourth symbol is used for the empty part of the volume slider
    /// Fifth symbol is used for the end boundary of the volume slider
    pub symbols: [String; 5],
    /// Style for the filled part of the volume slider
    /// Falls back to blue for foreground and default color for background
    pub filled_style: Style,
    /// Thumb at the end of the filled part of the volume slider
    /// Falls back to blue for foreground and default color for background
    pub thumb_style: Style,
    /// Style for the empty part of the volume slider
    /// Falls back to gray for foreground and default color for background
    pub track_style: Style,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VolumeSliderConfigFile {
    pub(super) symbols: Vec<String>,
    pub(super) track_style: Option<StyleFile>,
    pub(super) filled_style: Option<StyleFile>,
    pub(super) thumb_style: Option<StyleFile>,
}

impl Default for VolumeSliderConfigFile {
    fn default() -> Self {
        Self {
            symbols: vec![
                "♪".to_owned(),
                "─".to_owned(),
                "●".to_owned(),
                "─".to_owned(),
                "♫".to_owned(),
            ],
            filled_style: Some(StyleFile {
                fg: Some("blue".to_string()),
                bg: None,
                modifiers: None,
            }),
            thumb_style: Some(StyleFile {
                fg: Some("blue".to_string()),
                bg: None,
                modifiers: None,
            }),
            track_style: Some(StyleFile {
                fg: Some("dark_gray".to_string()),
                bg: None,
                modifiers: None,
            }),
        }
    }
}

impl VolumeSliderConfigFile {
    pub fn into_config(mut self) -> Result<VolumeSliderConfig> {
        let start = std::mem::take(&mut self.symbols[0]);
        let filled = std::mem::take(&mut self.symbols[1]);
        let thumb = std::mem::take(&mut self.symbols[2]);
        let track = std::mem::take(&mut self.symbols[3]);
        let end = std::mem::take(&mut self.symbols[4]);

        Ok(VolumeSliderConfig {
            symbols: [start, filled, thumb, track, end],
            filled_style: self.filled_style.to_config_or(Some(Color::Blue), None)?,
            thumb_style: self.thumb_style.to_config_or(Some(Color::Blue), None)?,
            track_style: self.track_style.to_config_or(Some(Color::DarkGray), None)?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::needless_pass_by_value)]
mod tests {
    use ratatui::style::Color as RC;
    use test_case::test_case;

    use crate::config::theme::{Style, StyleFile, volume_slider::VolumeSliderConfigFile};

    #[test]
    fn maps_symbols() {
        let input = VolumeSliderConfigFile {
            symbols: vec![
                "a".to_owned(),
                "b".to_owned(),
                "c".to_owned(),
                "d".to_owned(),
                "e".to_owned(),
            ],
            ..Default::default()
        };

        let result = input.into_config().unwrap().symbols;

        assert_eq!(result, [
            "a".to_owned(),
            "b".to_owned(),
            "c".to_owned(),
            "d".to_owned(),
            "e".to_owned()
        ]);
    }

    #[test_case(None,         None,         Style::default().fg(RC::Blue)                ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), Style::default().fg(RC::Blue)                ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), Style::default().fg(RC::Red).bg(RC::Blue)    ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         Style::default().fg(RC::Cyan)                ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), Style::default().fg(RC::Blue).bg(RC::Gray)   ; "correctly maps when only bg is provided")]
    fn filled_colors_test(c1: Option<&str>, c2: Option<&str>, expected: Style) {
        let input = VolumeSliderConfigFile {
            filled_style: match (c1, c2) {
                (Some("none"), Some("none")) => None,
                (Some(c1), Some(c2)) => Some(StyleFile {
                    fg: Some(c1.to_string()),
                    bg: Some(c2.to_string()),
                    modifiers: None,
                }),
                (Some(c1), None) => {
                    Some(StyleFile { fg: Some(c1.to_string()), bg: None, modifiers: None })
                }
                (None, Some(c2)) => {
                    Some(StyleFile { fg: None, bg: Some(c2.to_string()), modifiers: None })
                }
                (None, None) => Some(StyleFile { fg: None, bg: None, modifiers: None }),
            },
            ..Default::default()
        };

        let result = input.into_config().unwrap().filled_style;

        assert_eq!(result, expected);
    }

    #[test_case(None,         None,         Style::default().fg(RC::Blue)                ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), Style::default().fg(RC::Blue)                ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), Style::default().fg(RC::Red).bg(RC::Blue)    ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         Style::default().fg(RC::Cyan)                ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), Style::default().fg(RC::Blue).bg(RC::Gray)   ; "correctly maps when only bg is provided")]
    fn thumb_colors_test(c1: Option<&str>, c2: Option<&str>, expected: Style) {
        let input = VolumeSliderConfigFile {
            thumb_style: match (c1, c2) {
                (Some("none"), Some("none")) => None,
                (Some(c1), Some(c2)) => Some(StyleFile {
                    fg: Some(c1.to_string()),
                    bg: Some(c2.to_string()),
                    modifiers: None,
                }),
                (Some(c1), None) => {
                    Some(StyleFile { fg: Some(c1.to_string()), bg: None, modifiers: None })
                }
                (None, Some(c2)) => {
                    Some(StyleFile { fg: None, bg: Some(c2.to_string()), modifiers: None })
                }
                (None, None) => Some(StyleFile { fg: None, bg: None, modifiers: None }),
            },
            ..Default::default()
        };

        let result = input.into_config().unwrap().thumb_style;

        assert_eq!(result, expected);
    }

    #[test_case(None,         None,         Style::default().fg(RC::DarkGray)             ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), Style::default().fg(RC::DarkGray)             ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), Style::default().fg(RC::Red).bg(RC::Blue)     ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         Style::default().fg(RC::Cyan)                 ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), Style::default().fg(RC::DarkGray).bg(RC::Gray); "correctly maps when only bg is provided")]
    fn track_colors_test(c1: Option<&str>, c2: Option<&str>, expected: Style) {
        let input = VolumeSliderConfigFile {
            track_style: match (c1, c2) {
                (Some("none"), Some("none")) => None,
                (Some(c1), Some(c2)) => Some(StyleFile {
                    fg: Some(c1.to_string()),
                    bg: Some(c2.to_string()),
                    modifiers: None,
                }),
                (Some(c1), None) => {
                    Some(StyleFile { fg: Some(c1.to_string()), bg: None, modifiers: None })
                }
                (None, Some(c2)) => {
                    Some(StyleFile { fg: None, bg: Some(c2.to_string()), modifiers: None })
                }
                (None, None) => Some(StyleFile { fg: None, bg: None, modifiers: None }),
            },
            ..Default::default()
        };

        let result = input.into_config().unwrap().track_style;

        assert_eq!(result, expected);
    }
}
