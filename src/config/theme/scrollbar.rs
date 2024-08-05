use anyhow::Result;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};

use super::{style::ToConfigOr, StyleFile};

#[derive(Debug, Default, Clone, Copy)]
pub struct ScrollbarConfig {
    /// Symbols used for the scrollbar
    /// First symbol is used for the scrollbar track
    /// Second symbol is used for the scrollbar thumb
    /// Third symbol is used for the scrollbar up button
    /// Fourth symbol is used for the scrollbar down button
    pub symbols: [&'static str; 4],
    /// Fall sback to border color for foreground and default color for background
    pub track_style: Style,
    /// Fall sback to border color for foreground and default color for background
    pub ends_style: Style,
    // Falls back to blue for foreground and default color for background
    pub thumb_style: Style,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScrollbarConfigFile {
    pub(super) symbols: Vec<String>,
    pub(super) track_style: Option<StyleFile>,
    pub(super) ends_style: Option<StyleFile>,
    pub(super) thumb_style: Option<StyleFile>,
}

impl Default for ScrollbarConfigFile {
    fn default() -> Self {
        Self {
            symbols: vec!["│".to_owned(), "█".to_owned(), "▲".to_owned(), "▼".to_owned()],
            track_style: Some(StyleFile {
                fg: None,
                bg: None,
                modifiers: None,
            }),
            ends_style: Some(StyleFile {
                fg: None,
                bg: None,
                modifiers: None,
            }),
            thumb_style: Some(StyleFile {
                fg: Some("blue".to_string()),
                bg: None,
                modifiers: None,
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
            ends_style: self.ends_style.to_config_or(Some(fallback_color), None)?,
            thumb_style: self.thumb_style.to_config_or(Some(Color::Blue), None)?,
            track_style: self.track_style.to_config_or(Some(fallback_color), None)?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::needless_pass_by_value)]
mod tests {
    use crate::config::theme::{scrollbar::ScrollbarConfigFile, style::Modifiers, Style, StyleFile};
    use ratatui::style::{Color as RC, Modifier as RM};
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

    #[test_case(None,         None,         Style::default().fg(RC::Blue)                ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), Style::default().fg(RC::Blue)                ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), Style::default().fg(RC::Red).bg(RC::Blue)    ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         Style::default().fg(RC::Cyan)                ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), Style::default().fg(RC::Blue).bg(RC::Gray)   ; "correctly maps when only bg is provided")]
    fn thumb_colors_test(c1: Option<&str>, c2: Option<&str>, expected: Style) {
        let fallback = RC::DarkGray;
        let input = ScrollbarConfigFile {
            thumb_style: match (c1, c2) {
                (Some("none"), Some("none")) => None,
                (Some(c1), Some(c2)) => Some(StyleFile {
                    fg: Some(c1.to_string()),
                    bg: Some(c2.to_string()),
                    modifiers: None,
                }),
                (Some(c1), None) => Some(StyleFile {
                    fg: Some(c1.to_string()),
                    bg: None,
                    modifiers: None,
                }),
                (None, Some(c2)) => Some(StyleFile {
                    fg: None,
                    bg: Some(c2.to_string()),
                    modifiers: None,
                }),
                (None, None) => Some(StyleFile {
                    fg: None,
                    bg: None,
                    modifiers: None,
                }),
            },
            ..Default::default()
        };

        let result = input.into_config(fallback).unwrap();

        assert_eq!(result.thumb_style, expected);
    }

    #[test_case(None,         None,         Style::default().fg(RC::DarkGray)                ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), Style::default().fg(RC::DarkGray)                ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), Style::default().fg(RC::Red).bg(RC::Blue)        ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         Style::default().fg(RC::Cyan)                    ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), Style::default().fg(RC::DarkGray).bg(RC::Gray)   ; "correctly maps when only bg is provided")]
    fn ends_colors_test(c1: Option<&str>, c2: Option<&str>, expected: Style) {
        let fallback = RC::DarkGray;
        let input = ScrollbarConfigFile {
            ends_style: match (c1, c2) {
                (Some("none"), Some("none")) => None,
                (Some(c1), Some(c2)) => Some(StyleFile {
                    fg: Some(c1.to_string()),
                    bg: Some(c2.to_string()),
                    modifiers: None,
                }),
                (Some(c1), None) => Some(StyleFile {
                    fg: Some(c1.to_string()),
                    bg: None,
                    modifiers: None,
                }),
                (None, Some(c2)) => Some(StyleFile {
                    fg: None,
                    bg: Some(c2.to_string()),
                    modifiers: None,
                }),
                (None, None) => Some(StyleFile {
                    fg: None,
                    bg: None,
                    modifiers: None,
                }),
            },
            ..Default::default()
        };

        let result = input.into_config(fallback).unwrap();

        assert_eq!(result.ends_style, expected);
    }

    #[test_case(None,         None,         Style::default().fg(RC::DarkGray)                ; "uses default colors")]
    #[test_case(Some("none"), Some("none"), Style::default().fg(RC::DarkGray)                ; "uses default colors when whole value is None")]
    #[test_case(Some("red"),  Some("blue"), Style::default().fg(RC::Red).bg(RC::Blue)        ; "correctly maps provided colors")]
    #[test_case(Some("cyan"), None,         Style::default().fg(RC::Cyan)                    ; "correctly maps when only fg is provided")]
    #[test_case(None,         Some("gray"), Style::default().fg(RC::DarkGray).bg(RC::Gray)   ; "correctly maps when only bg is provided")]
    fn track_colors_test(c1: Option<&str>, c2: Option<&str>, expected: Style) {
        let fallback = RC::DarkGray;
        let input = ScrollbarConfigFile {
            track_style: match (c1, c2) {
                (Some("none"), Some("none")) => None,
                (Some(c1), Some(c2)) => Some(StyleFile {
                    fg: Some(c1.to_string()),
                    bg: Some(c2.to_string()),
                    modifiers: None,
                }),
                (Some(c1), None) => Some(StyleFile {
                    fg: Some(c1.to_string()),
                    bg: None,
                    modifiers: None,
                }),
                (None, Some(c2)) => Some(StyleFile {
                    fg: None,
                    bg: Some(c2.to_string()),
                    modifiers: None,
                }),
                (None, None) => Some(StyleFile {
                    fg: None,
                    bg: None,
                    modifiers: None,
                }),
            },
            ..Default::default()
        };

        let result = input.into_config(fallback).unwrap();

        assert_eq!(result.track_style, expected);
    }

    #[test_case(Modifiers::Bold,       RM::BOLD; "bold")]
    #[test_case(Modifiers::Dim,        RM::DIM; "dim")]
    #[test_case(Modifiers::Italic,     RM::ITALIC; "italic")]
    #[test_case(Modifiers::Underlined, RM::UNDERLINED; "underlined")]
    #[test_case(Modifiers::Reversed,   RM::REVERSED; "reversed")]
    #[test_case(Modifiers::CrossedOut, RM::CROSSED_OUT; "crossed out")]
    fn thumb_modifiers(input: Modifiers, expected: RM) {
        let input = ScrollbarConfigFile {
            thumb_style: Some(StyleFile {
                fg: None,
                bg: None,
                modifiers: Some(input),
            }),
            ..Default::default()
        };

        let result = input.into_config(RC::Blue).unwrap();

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
    fn ends_modifiers(input: Modifiers, expected: RM) {
        let input = ScrollbarConfigFile {
            ends_style: Some(StyleFile {
                fg: None,
                bg: None,
                modifiers: Some(input),
            }),
            ..Default::default()
        };

        let result = input.into_config(RC::Blue).unwrap();

        assert_eq!(
            result.ends_style.add_modifier,
            Style::default().add_modifier(expected).add_modifier
        );
    }

    #[test_case(Modifiers::Bold,       RM::BOLD; "bold")]
    #[test_case(Modifiers::Dim,        RM::DIM; "dim")]
    #[test_case(Modifiers::Italic,     RM::ITALIC; "italic")]
    #[test_case(Modifiers::Underlined, RM::UNDERLINED; "underlined")]
    #[test_case(Modifiers::Reversed,   RM::REVERSED; "reversed")]
    #[test_case(Modifiers::CrossedOut, RM::CROSSED_OUT; "crossed out")]
    fn track_modifiers(input: Modifiers, expected: RM) {
        let input = ScrollbarConfigFile {
            track_style: Some(StyleFile {
                fg: None,
                bg: None,
                modifiers: Some(input),
            }),
            ..Default::default()
        };

        let result = input.into_config(RC::Blue).unwrap();

        assert_eq!(
            result.track_style.add_modifier,
            Style::default().add_modifier(expected).add_modifier
        );
    }
}
