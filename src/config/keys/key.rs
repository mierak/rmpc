use std::{fmt::Display, str::FromStr};

use crossterm::event::{KeyCode, KeyModifiers};
use itertools::Itertools;
use serde_with::{DeserializeFromStr, SerializeDisplay};

#[derive(Debug, SerializeDisplay, DeserializeFromStr, PartialEq, Eq, Hash, Clone)]
pub struct Key {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
}

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let has_ctrl = self.modifiers.contains(KeyModifiers::CONTROL);
        let has_alt = self.modifiers.contains(KeyModifiers::ALT);
        let has_shift = self.modifiers.contains(KeyModifiers::SHIFT);
        let has_no_modifiers = !has_ctrl && !has_alt && !has_shift;

        if has_ctrl
            || has_alt
            || (has_shift && !matches!(self.key, KeyCode::Char(c) if c.is_alphabetic()))
        {
            write!(f, "<")?;
        }
        if has_ctrl {
            write!(f, "C-")?;
        }
        if has_alt {
            write!(f, "A-")?;
        }
        if has_shift && !matches!(self.key, KeyCode::Char(c) if c.is_alphabetic()) {
            write!(f, "S-")?;
        }

        match self.key {
            KeyCode::Backspace if has_no_modifiers => write!(f, "<BS>"),
            KeyCode::Backspace => write!(f, "BS"),
            KeyCode::Enter if has_no_modifiers => write!(f, "<CR>"),
            KeyCode::Enter => write!(f, "CR"),
            KeyCode::Left if has_no_modifiers => write!(f, "<Left>"),
            KeyCode::Left => write!(f, "Left"),
            KeyCode::Right if has_no_modifiers => write!(f, "<Right>"),
            KeyCode::Right => write!(f, "Right"),
            KeyCode::Up if has_no_modifiers => write!(f, "<Up>"),
            KeyCode::Up => write!(f, "Up"),
            KeyCode::Down if has_no_modifiers => write!(f, "<Down>"),
            KeyCode::Down => write!(f, "Down"),
            KeyCode::Home if has_no_modifiers => write!(f, "<Home>"),
            KeyCode::Home => write!(f, "Home"),
            KeyCode::End if has_no_modifiers => write!(f, "<End>"),
            KeyCode::End => write!(f, "End"),
            KeyCode::PageUp if has_no_modifiers => write!(f, "<PageUp>"),
            KeyCode::PageUp => write!(f, "PageUp"),
            KeyCode::PageDown if has_no_modifiers => write!(f, "<PageDown>"),
            KeyCode::PageDown => write!(f, "PageDown"),
            KeyCode::Tab if has_no_modifiers => write!(f, "<Tab>"),
            KeyCode::Tab => write!(f, "Tab"),
            KeyCode::BackTab if has_no_modifiers => write!(f, "<Tab>"),
            KeyCode::BackTab => write!(f, "Tab"),
            KeyCode::Delete if has_no_modifiers => write!(f, "<Del>"),
            KeyCode::Delete => write!(f, "Del"),
            KeyCode::Insert if has_no_modifiers => write!(f, "<Insert>"),
            KeyCode::Insert => write!(f, "Insert"),
            KeyCode::Esc if has_no_modifiers => write!(f, "<Esc>"),
            KeyCode::Esc => write!(f, "Esc"),
            KeyCode::F(num) if has_no_modifiers => write!(f, "<F{num}>"),
            KeyCode::F(num) => write!(f, "F{num}"),
            KeyCode::Char(' ') if has_no_modifiers => write!(f, "<Space>"),
            KeyCode::Char(' ') => write!(f, "Space"),
            KeyCode::Char(char) => write!(f, "{char}"),
            KeyCode::CapsLock
            | KeyCode::ScrollLock
            | KeyCode::NumLock
            | KeyCode::PrintScreen
            | KeyCode::Pause
            | KeyCode::Menu
            | KeyCode::KeypadBegin
            | KeyCode::Media(_)
            | KeyCode::Modifier(_)
            | KeyCode::Null => Ok(()),
        }?;

        if has_ctrl
            || has_alt
            || (has_shift && !matches!(self.key, KeyCode::Char(c) if c.is_alphabetic()))
        {
            write!(f, ">")?;
        }

        Ok(())
    }
}

impl FromStr for Key {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chars = s.chars().collect_vec();
        let mut modifiers = KeyModifiers::NONE;
        let mut i = 0;

        let mut key_part_range = 0..chars.len();
        loop {
            let Some(c) = chars.get(i) else {
                break;
            };
            let next = chars.get(i + 1);

            match c {
                'C' if next.is_some_and(|v| v == &'-') => {
                    modifiers |= KeyModifiers::CONTROL;
                    i += 1;
                }
                'A' if next.is_some_and(|v| v == &'-') => {
                    modifiers |= KeyModifiers::ALT;
                    i += 1;
                }
                'S' if next.is_some_and(|v| v == &'-') => {
                    modifiers |= KeyModifiers::SHIFT;
                    i += 1;
                }
                '<' if next.is_some_and(|v| v != &'>') => {} // skip, is prefix
                '>' if next.is_none() && chars.len() > 1 => {
                    // is suffix, end
                    key_part_range.end = i;
                    break;
                }
                _ if key_part_range.start == 0 => {
                    key_part_range.start = i;
                }
                _ => {}
            }
            i += 1;
        }

        let key_part = &chars[key_part_range];
        let key = match key_part.iter().collect::<String>().as_str() {
            "BS" => KeyCode::Backspace,
            "Backspace" => KeyCode::Backspace,
            "CR" => KeyCode::Enter,
            "Enter" => KeyCode::Enter,
            "Left" => KeyCode::Left,
            "Right" => KeyCode::Right,
            "Up" => KeyCode::Up,
            "Down" => KeyCode::Down,
            "Home" => KeyCode::Home,
            "End" => KeyCode::End,
            "PageUp" => KeyCode::PageUp,
            "PageDown" => KeyCode::PageDown,
            "Tab" if modifiers.contains(KeyModifiers::SHIFT) => KeyCode::BackTab,
            "Tab" => KeyCode::Tab,
            "Del" => KeyCode::Delete,
            "Insert" => KeyCode::Insert,
            "Esc" => KeyCode::Esc,
            "Space" => KeyCode::Char(' '),
            "F1" => KeyCode::F(1),
            "F2" => KeyCode::F(2),
            "F3" => KeyCode::F(3),
            "F4" => KeyCode::F(4),
            "F5" => KeyCode::F(5),
            "F6" => KeyCode::F(6),
            "F7" => KeyCode::F(7),
            "F8" => KeyCode::F(8),
            "F9" => KeyCode::F(9),
            "F10" => KeyCode::F(10),
            "F11" => KeyCode::F(11),
            "F12" => KeyCode::F(12),
            "" => KeyCode::Null,
            c => {
                if key_part.len() != 1 {
                    return Err(format!("Invalid key: '{c}' from input '{s}'"));
                }

                if key_part[0].is_uppercase() {
                    modifiers |= KeyModifiers::SHIFT;
                }

                KeyCode::Char(key_part[0])
            }
        };

        Ok(Self { key, modifiers })
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("a",            Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::NONE })]
    #[case("A",            Key { key: KeyCode::Char('A'), modifiers: KeyModifiers::SHIFT })]
    #[case("c",            Key { key: KeyCode::Char('c'), modifiers: KeyModifiers::NONE })]
    #[case("C",            Key { key: KeyCode::Char('C'), modifiers: KeyModifiers::SHIFT })]
    #[case("<C-a>",        Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL })]
    #[case("<C-A>",        Key { key: KeyCode::Char('A'), modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT })]
    #[case("<C-S-Tab>",    Key { key: KeyCode::BackTab,   modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT })]
    #[case("<S-Tab>",      Key { key: KeyCode::BackTab,   modifiers: KeyModifiers::SHIFT })]
    #[case("<C-Tab>",      Key { key: KeyCode::Tab,       modifiers: KeyModifiers::CONTROL })]
    #[case("<Tab>",        Key { key: KeyCode::Tab,       modifiers: KeyModifiers::NONE })]
    #[case("5",            Key { key: KeyCode::Char('5'), modifiers: KeyModifiers::NONE })]
    #[case("<C-A-S-5>",    Key { key: KeyCode::Char('5'), modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT | KeyModifiers::ALT })]
    #[case("<C-Space>",    Key { key: KeyCode::Char(' '), modifiers: KeyModifiers::CONTROL })]
    #[case("<Space>",      Key { key: KeyCode::Char(' '), modifiers: KeyModifiers::NONE })]
    #[case("<C-S-Space>",  Key { key: KeyCode::Char(' '), modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT })]
    #[case("<C-S-F11>",    Key { key: KeyCode::F(11),     modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT })]
    #[case("<F11>",        Key { key: KeyCode::F(11),     modifiers: KeyModifiers::NONE })]
    #[case("<BS>",         Key { key: KeyCode::Backspace, modifiers: KeyModifiers::NONE })]
    #[case("<C-BS>",       Key { key: KeyCode::Backspace, modifiers: KeyModifiers::CONTROL })]
    #[case("<CR>",         Key { key: KeyCode::Enter,     modifiers: KeyModifiers::NONE })]
    #[case("<C-CR>",       Key { key: KeyCode::Enter,     modifiers: KeyModifiers::CONTROL })]
    #[case("<Left>",       Key { key: KeyCode::Left,      modifiers: KeyModifiers::NONE })]
    #[case("<C-Left>",     Key { key: KeyCode::Left,      modifiers: KeyModifiers::CONTROL })]
    #[case("<Right>",      Key { key: KeyCode::Right,     modifiers: KeyModifiers::NONE })]
    #[case("<C-Right>",    Key { key: KeyCode::Right,     modifiers: KeyModifiers::CONTROL })]
    #[case("<Up>",         Key { key: KeyCode::Up,        modifiers: KeyModifiers::NONE })]
    #[case("<C-Up>",       Key { key: KeyCode::Up,        modifiers: KeyModifiers::CONTROL })]
    #[case("<Down>",       Key { key: KeyCode::Down,      modifiers: KeyModifiers::NONE })]
    #[case("<C-Down>",     Key { key: KeyCode::Down,      modifiers: KeyModifiers::CONTROL })]
    #[case("<Home>",       Key { key: KeyCode::Home,      modifiers: KeyModifiers::NONE })]
    #[case("<C-Home>",     Key { key: KeyCode::Home,      modifiers: KeyModifiers::CONTROL })]
    #[case("<End>",        Key { key: KeyCode::End,       modifiers: KeyModifiers::NONE })]
    #[case("<C-End>",      Key { key: KeyCode::End,       modifiers: KeyModifiers::CONTROL })]
    #[case("<PageUp>",     Key { key: KeyCode::PageUp,    modifiers: KeyModifiers::NONE })]
    #[case("<C-PageUp>",   Key { key: KeyCode::PageUp,    modifiers: KeyModifiers::CONTROL })]
    #[case("<PageDown>",   Key { key: KeyCode::PageDown,  modifiers: KeyModifiers::NONE })]
    #[case("<C-PageDown>", Key { key: KeyCode::PageDown,  modifiers: KeyModifiers::CONTROL })]
    #[case("<Del>",        Key { key: KeyCode::Delete,    modifiers: KeyModifiers::NONE })]
    #[case("<C-Del>",      Key { key: KeyCode::Delete,    modifiers: KeyModifiers::CONTROL })]
    #[case("<Esc>",        Key { key: KeyCode::Esc,       modifiers: KeyModifiers::NONE })]
    #[case("<C-Esc>",      Key { key: KeyCode::Esc,       modifiers: KeyModifiers::CONTROL })]
    #[case(">",            Key { key: KeyCode::Char('>'), modifiers: KeyModifiers::NONE })]
    #[case("<",            Key { key: KeyCode::Char('<'), modifiers: KeyModifiers::NONE })]
    #[case("<C-S-<>",      Key { key: KeyCode::Char('<'), modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT })]
    #[case("_",            Key { key: KeyCode::Char('_'), modifiers: KeyModifiers::NONE })]
    #[case("-",            Key { key: KeyCode::Char('-'), modifiers: KeyModifiers::NONE })]
    #[case("<C-S-->",      Key { key: KeyCode::Char('-'), modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT })]
    #[case("5",            Key { key: KeyCode::Char('5'), modifiers: KeyModifiers::NONE })]
    #[case("%",            Key { key: KeyCode::Char('%'), modifiers: KeyModifiers::NONE })]
    #[case("",             Key { key: KeyCode::Null,      modifiers: KeyModifiers::NONE })]
    fn serialization_round_trip(#[case] expected_str: &str, #[case] input: Key) {
        let serialized = input.to_string();
        assert_eq!(expected_str, serialized);

        let deserialized: Key = serialized.parse().unwrap();
        assert_eq!(deserialized, input);
    }

    #[rstest]
    #[case("<Enter>",          Key { key: KeyCode::Enter,       modifiers: KeyModifiers::NONE })]
    #[case("<Backspace>",      Key { key: KeyCode::Backspace,   modifiers: KeyModifiers::NONE })]
    fn deserialization_extras(#[case] input: &str, #[case] expected: Key) {
        let deserialized: Key = input.parse().unwrap();
        assert_eq!(deserialized, expected);
    }
}
