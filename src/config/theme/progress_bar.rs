use anyhow::Result;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};

use super::{StyleFile, style::ToConfigOr};

#[derive(Debug, Default, Clone)]
pub struct ProgressBarConfig {
    /// Symbols for the rogress bar at the bottom of the screen
    /// First symbol is used for the start boundary of the progress bar
    /// Second symbol is used for the elapsed part of the progress bar
    /// Third symbol is used for the thumb
    /// Fourth symbol is used for the remaining part of the progress bar
    /// Fifth symbol is used for the end boundary of the progress bar
    pub symbols: [String; 5],
    /// Fall sback to blue for foreground and black for background
    pub elapsed_style: Style,
    /// Thumb at the end of the elapsed part of the progress bar
    /// Fall sback to blue for foreground and black for background
    pub thumb_style: Style,
    /// Fall sback to black for foreground and default color for background
    /// For transparent track you should set the track symbol to empty string
    pub track_style: Style,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProgressBarConfigFile {
    pub(super) symbols: Vec<String>,
    pub(super) track_style: Option<StyleFile>,
    pub(super) elapsed_style: Option<StyleFile>,
    pub(super) thumb_style: Option<StyleFile>,
}

impl Default for ProgressBarConfigFile {
    fn default() -> Self {
        Self {
            symbols: vec![
                "[".to_owned(),
                "-".to_owned(),
                ">".to_owned(),
                " ".to_owned(),
                "]".to_owned(),
            ],
            elapsed_style: Some(StyleFile {
                fg: Some("blue".to_string()),
                bg: None,
                modifiers: None,
            }),
            thumb_style: Some(StyleFile {
                fg: Some("blue".to_string()),
                bg: Some("#1e2030".to_string()),
                modifiers: None,
            }),
            track_style: Some(StyleFile {
                fg: Some("#1e2030".to_string()),
                bg: None,
                modifiers: None,
            }),
        }
    }
}

impl ProgressBarConfigFile {
    pub(super) fn into_config(mut self) -> Result<ProgressBarConfig> {
        if self.symbols.len() == 3 {
            self.symbols.resize(5, String::default());
            let s0 = self.symbols[0].clone();
            let s1 = self.symbols[1].clone();
            let s2 = self.symbols[2].clone();
            let s3 = s2.clone();

            self.symbols[1] = s0;
            self.symbols[2] = s1;
            self.symbols[3] = s2;
            self.symbols[4] = s3;
        }
        let start = std::mem::take(&mut self.symbols[0]);
        let elapsed = std::mem::take(&mut self.symbols[1]);
        let thumb = std::mem::take(&mut self.symbols[2]);
        let track = std::mem::take(&mut self.symbols[3]);
        let end = std::mem::take(&mut self.symbols[4]);

        Ok(ProgressBarConfig {
            symbols: [start, elapsed, thumb, track, end],
            elapsed_style: self.elapsed_style.to_config_or(Some(Color::Blue), None)?,
            thumb_style: self.thumb_style.to_config_or(Some(Color::Blue), None)?,
            track_style: self.track_style.to_config_or(Some(Color::Black), None)?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::needless_pass_by_value)]
mod tests {
    use ratatui::style::{Color as RC, Modifier as RM};
    use test_case::test_case;

    use crate::config::theme::{
        Style,
        StyleFile,
        progress_bar::ProgressBarConfigFile,
        style::Modifiers,
    };

    #[test]
    fn maps_symbols() {
        let input = ProgressBarConfigFile {
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
    fn elapsed_colors_test(c1: Option<&str>, c2: Option<&str>, expected: Style) {
        let input = ProgressBarConfigFile {
            elapsed_style: match (c1, c2) {
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

        let result = input.into_config().unwrap();

        assert_eq!(result.elapsed_style, expected);
    }

    #[test_case(None,         None,         Style::default().fg(RC::Black)                ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), Style::default().fg(RC::Black)                ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), Style::default().fg(RC::Red).bg(RC::Blue)     ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         Style::default().fg(RC::Cyan)                 ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), Style::default().fg(RC::Black).bg(RC::Gray)   ; "correctly maps when only bg is provided")]
    fn track_colors_test(c1: Option<&str>, c2: Option<&str>, expected: Style) {
        let input = ProgressBarConfigFile {
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

        let result = input.into_config().unwrap();

        assert_eq!(result.track_style, expected);
    }

    #[test_case(None,         None,         Style::default().fg(RC::Blue)                ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), Style::default().fg(RC::Blue)                ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), Style::default().fg(RC::Red).bg(RC::Blue)    ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         Style::default().fg(RC::Cyan)                ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), Style::default().fg(RC::Blue).bg(RC::Gray)   ; "correctly maps when only bg is provided")]
    fn thumb_colors_test(c1: Option<&str>, c2: Option<&str>, expected: Style) {
        let input = ProgressBarConfigFile {
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

        let result = input.into_config().unwrap();

        assert_eq!(result.thumb_style, expected);
    }

    #[test_case(Modifiers::Bold,       RM::BOLD; "bold")]
    #[test_case(Modifiers::Dim,        RM::DIM; "dim")]
    #[test_case(Modifiers::Italic,     RM::ITALIC; "italic")]
    #[test_case(Modifiers::Underlined, RM::UNDERLINED; "underlined")]
    #[test_case(Modifiers::Reversed,   RM::REVERSED; "reversed")]
    #[test_case(Modifiers::CrossedOut, RM::CROSSED_OUT; "crossed out")]
    fn track_modifiers(input: Modifiers, expected: RM) {
        let input = ProgressBarConfigFile {
            track_style: Some(StyleFile { fg: None, bg: None, modifiers: Some(input) }),
            ..Default::default()
        };

        let result = input.into_config().unwrap();

        assert_eq!(
            result.track_style.add_modifier,
            Style::default().add_modifier(expected).add_modifier
        );
    }

    #[test_case(Modifiers::Bold,       RM::BOLD; "bold")]
    #[test_case(Modifiers::Dim,        RM::DIM; "dim")]
    #[test_case(Modifiers::Italic,     RM::ITALIC; "italic")]
    #[test_case(Modifiers::Underlined, RM::UNDERLINED; "underlined")]
    #[test_case(Modifiers::Reversed,   RM::REVERSED; "reversed")]
    #[test_case(Modifiers::CrossedOut, RM::CROSSED_OUT; "crossed out")]
    fn thumb_modifiers(input: Modifiers, expected: RM) {
        let input = ProgressBarConfigFile {
            thumb_style: Some(StyleFile { fg: None, bg: None, modifiers: Some(input) }),
            ..Default::default()
        };

        let result = input.into_config().unwrap();

        assert_eq!(
            result.thumb_style.add_modifier,
            Style::default().add_modifier(expected).add_modifier
        );
    }

    #[test_case(Modifiers::Bold,       RM::BOLD; "bold")]
    #[test_case(Modifiers::Dim,        RM::DIM; "dim")]
    #[test_case(Modifiers::Italic,     RM::ITALIC; "italic")]
    #[test_case(Modifiers::Underlined, RM::UNDERLINED; "underlined")]
    #[test_case(Modifiers::Reversed,   RM::REVERSED; "reversed")]
    #[test_case(Modifiers::CrossedOut, RM::CROSSED_OUT; "crossed out")]
    fn elapsed_modifiers(input: Modifiers, expected: RM) {
        let input = ProgressBarConfigFile {
            elapsed_style: Some(StyleFile { fg: None, bg: None, modifiers: Some(input) }),
            ..Default::default()
        };

        let result = input.into_config().unwrap();

        assert_eq!(
            result.elapsed_style.add_modifier,
            Style::default().add_modifier(expected).add_modifier
        );
    }
}
