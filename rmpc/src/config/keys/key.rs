use std::{fmt::Display, str::FromStr};

use crossterm::event::{KeyCode, KeyEvent as CKeyEvent, KeyModifiers};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use winnow::{
    Parser,
    Result,
    combinator::{alt, dispatch, empty, fail, opt, permutation, repeat, seq, trace},
    token::{any, literal},
};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct Key {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
}

impl From<CKeyEvent> for Key {
    fn from(value: CKeyEvent) -> Self {
        let should_insert_shift = matches!(value.code, KeyCode::Char(c) if c.is_uppercase());

        let mut modifiers = value.modifiers;
        if should_insert_shift {
            modifiers.insert(KeyModifiers::SHIFT);
        }

        let key = if modifiers.contains(KeyModifiers::SHIFT) {
            if let KeyCode::Char(c) = value.code {
                KeyCode::Char(c.to_ascii_uppercase())
            } else {
                value.code
            }
        } else {
            value.code
        };

        Self { key, modifiers }
    }
}

#[derive(
    Debug,
    SerializeDisplay,
    DeserializeFromStr,
    PartialEq,
    Eq,
    Hash,
    Clone,
    derive_more::IntoIterator,
)]
pub struct KeySequence(pub Vec<Key>);

impl KeySequence {
    pub fn iter(&self) -> impl Iterator<Item = &Key> {
        let mut iter = self.0.iter();
        std::iter::from_fn(move || iter.next())
    }

    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn char(mut self, c: char) -> Self {
        let key = if c.is_uppercase() {
            Key { key: KeyCode::Char(c.to_ascii_uppercase()), modifiers: KeyModifiers::SHIFT }
        } else {
            Key { key: KeyCode::Char(c), modifiers: KeyModifiers::NONE }
        };
        self.0.push(key);
        self
    }

    pub fn ctrl(mut self) -> Self {
        if let Some(last_key) = self.0.last_mut() {
            last_key.modifiers |= KeyModifiers::CONTROL;
        }
        self
    }

    pub fn shift(mut self) -> Self {
        if let Some(last_key) = self.0.last_mut()
            && !matches!(last_key.key, KeyCode::Char(_))
        {
            if matches!(last_key.key, KeyCode::Tab) {
                last_key.key = KeyCode::BackTab;
            }
            last_key.modifiers |= KeyModifiers::SHIFT;
        }
        self
    }

    pub fn tab(mut self) -> Self {
        self.0.push(Key { key: KeyCode::Tab, modifiers: KeyModifiers::NONE });
        self
    }

    pub fn cr(mut self) -> Self {
        self.0.push(Key { key: KeyCode::Enter, modifiers: KeyModifiers::NONE });
        self
    }

    pub fn up(mut self) -> Self {
        self.0.push(Key { key: KeyCode::Up, modifiers: KeyModifiers::NONE });
        self
    }

    pub fn down(mut self) -> Self {
        self.0.push(Key { key: KeyCode::Down, modifiers: KeyModifiers::NONE });
        self
    }

    pub fn left(mut self) -> Self {
        self.0.push(Key { key: KeyCode::Left, modifiers: KeyModifiers::NONE });
        self
    }

    pub fn right(mut self) -> Self {
        self.0.push(Key { key: KeyCode::Right, modifiers: KeyModifiers::NONE });
        self
    }

    pub fn esc(mut self) -> Self {
        self.0.push(Key { key: KeyCode::Esc, modifiers: KeyModifiers::NONE });
        self
    }

    pub fn page_up(mut self) -> Self {
        self.0.push(Key { key: KeyCode::PageUp, modifiers: KeyModifiers::NONE });
        self
    }

    pub fn page_down(mut self) -> Self {
        self.0.push(Key { key: KeyCode::PageDown, modifiers: KeyModifiers::NONE });
        self
    }
}

impl From<Key> for KeySequence {
    fn from(key: Key) -> Self {
        Self(vec![key])
    }
}

impl From<Vec<Key>> for KeySequence {
    fn from(keys: Vec<Key>) -> Self {
        Self(keys)
    }
}

impl Display for KeySequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.iter().try_for_each(|key| write!(f, "{key}"))
    }
}

impl FromStr for KeySequence {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let keys = parse_sequence.parse(s).map_err(|e| anyhow::format_err!("{e}"))?;
        Ok(Self(keys))
    }
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

fn parse_sequence(input: &mut &str) -> winnow::error::Result<Vec<Key>> {
    repeat(1.., parse_key).parse_next(input)
}

fn parse_key(input: &mut &str) -> winnow::error::Result<Key> {
    let ((modifiers, key),) = alt((
        trace("with modifiers or special key", seq! {
            _: '<',
            |input: &mut &str| {
                let mut mods = parse_modifier.parse_next(input)?;
                match alt((parse_special_key, trace("char", parse_char_key))).parse_next(input) {
                    Ok((mods2, mut key)) => {
                        mods |= mods2;

                        if mods.contains(KeyModifiers::SHIFT) && matches!(key, KeyCode::Tab){
                            key = KeyCode::BackTab;
                        }

                        Ok((mods, key))
                    },
                    Err(err) => {
                        return Err(err);
                    },
                }
            },
            _: '>'
        }),
        trace("single char key", parse_char_key.map(|v| (v,))),
    ))
    .parse_next(input)?;

    Ok(Key { key, modifiers })
}

fn parse_modifier(input: &mut &str) -> winnow::error::Result<KeyModifiers> {
    let mods = permutation((
        opt(literal("C-").value(KeyModifiers::CONTROL)),
        opt(literal("A-").value(KeyModifiers::ALT)),
        opt(literal("S-").value(KeyModifiers::SHIFT)),
    ))
    .parse_next(input)?;

    let mut modifiers = KeyModifiers::NONE;
    for modifier in [mods.0, mods.1, mods.2] {
        match modifier {
            Some(KeyModifiers::CONTROL) => modifiers |= KeyModifiers::CONTROL,
            Some(KeyModifiers::ALT) => modifiers |= KeyModifiers::ALT,
            Some(KeyModifiers::SHIFT) => modifiers |= KeyModifiers::SHIFT,
            _ => {}
        }
    }

    Ok(modifiers)
}

fn parse_char_key(input: &mut &str) -> Result<(KeyModifiers, KeyCode)> {
    let c = any.parse_next(input)?;
    if c.is_uppercase() {
        Ok((KeyModifiers::SHIFT, KeyCode::Char(c.to_ascii_uppercase())))
    } else {
        Ok((KeyModifiers::NONE, KeyCode::Char(c)))
    }
}

fn parse_special_key(input: &mut &str) -> winnow::error::Result<(KeyModifiers, KeyCode)> {
    let mut parser = alt((
        alt((
            "BS",
            "Backspace",
            "CR",
            "Enter",
            "Left",
            "Right",
            "Up",
            "Down",
            "Home",
            "End",
            "PageUp",
            "PageDown",
            "Tab",
        )),
        alt((
            "Del", "Insert", "Esc", "Space", "F10", "F11", "F12", "F1", "F2", "F3", "F4", "F5",
            "F6", "F7", "F8", "F9",
        )),
    ));

    let mut parser = dispatch! {parser;
        "BS" => empty.value(KeyCode::Backspace),
        "Backspace" => empty.value(KeyCode::Backspace),
        "CR" => empty.value(KeyCode::Enter),
        "Enter" => empty.value(KeyCode::Enter),
        "Left" => empty.value(KeyCode::Left),
        "Right" => empty.value(KeyCode::Right),
        "Up" => empty.value(KeyCode::Up),
        "Down" => empty.value(KeyCode::Down),
        "Home" => empty.value(KeyCode::Home),
        "End" => empty.value(KeyCode::End),
        "PageUp" => empty.value(KeyCode::PageUp),
        "PageDown" => empty.value(KeyCode::PageDown),
        "Tab" => empty.value(KeyCode::Tab),
        "Del" => empty.value(KeyCode::Delete),
        "Insert" => empty.value(KeyCode::Insert),
        "Esc" => empty.value(KeyCode::Esc),
        "Space" => empty.value(KeyCode::Char(' ')),
        "F10" => empty.value(KeyCode::F(10)),
        "F11" => empty.value(KeyCode::F(11)),
        "F12" => empty.value(KeyCode::F(12)),
        "F1" => empty.value(KeyCode::F(1)),
        "F2" => empty.value(KeyCode::F(2)),
        "F3" => empty.value(KeyCode::F(3)),
        "F4" => empty.value(KeyCode::F(4)),
        "F5" => empty.value(KeyCode::F(5)),
        "F6" => empty.value(KeyCode::F(6)),
        "F7" => empty.value(KeyCode::F(7)),
        "F8" => empty.value(KeyCode::F(8)),
        "F9" => empty.value(KeyCode::F(9)),
        "" => empty.value(KeyCode::Null),
        _ => fail,
    };

    parser.parse_next(input).map(|key| (KeyModifiers::NONE, key))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use itertools::Itertools;
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
    fn serialization_round_trip(#[case] expected_str: &str, #[case] input: Key) {
        let serialized = input.to_string();
        assert_eq!(expected_str, serialized);

        let deserialized: KeySequence = serialized.parse().unwrap();
        assert_eq!(deserialized.0[0], input);
    }

    #[rstest]
    #[case("acd",                  vec![Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::NONE }, Key { key: KeyCode::Char('c'), modifiers: KeyModifiers::NONE }, Key { key: KeyCode::Char('d'), modifiers: KeyModifiers::NONE }].into())]
    #[case("d<C-A>f",              vec![Key { key: KeyCode::Char('d'), modifiers: KeyModifiers::NONE }, Key { key: KeyCode::Char('A'), modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT }, Key { key: KeyCode::Char('f'), modifiers: KeyModifiers::NONE } ].into())]
    #[case("<C-<><C-a><C->>",      vec![Key { key: KeyCode::Char('<'), modifiers: KeyModifiers::CONTROL }, Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL }, Key { key: KeyCode::Char('>'), modifiers: KeyModifiers::CONTROL } ].into())]
    #[case("d<C-<>a<C-a>f<C->>t",  vec![Key { key: KeyCode::Char('d'), modifiers: KeyModifiers::NONE }, Key { key: KeyCode::Char('<'), modifiers: KeyModifiers::CONTROL }, Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::NONE }, Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL }, Key { key: KeyCode::Char('f'), modifiers: KeyModifiers::NONE }, Key { key: KeyCode::Char('>'), modifiers: KeyModifiers::CONTROL }, Key { key: KeyCode::Char('t'), modifiers: KeyModifiers::NONE } ].into())]
    fn sequence_round_trip(#[case] expected_str: &str, #[case] input: KeySequence) {
        let serialized = input.iter().join("");
        assert_eq!(expected_str, serialized);

        let deserialized: KeySequence = serialized.parse().unwrap();
        assert_eq!(deserialized, input);
    }

    #[rstest]
    #[case("BS")]
    #[case("Backspace")]
    #[case("CR")]
    #[case("Enter")]
    #[case("Left")]
    #[case("Right")]
    #[case("Up")]
    #[case("Down")]
    #[case("Home")]
    #[case("End")]
    #[case("PageUp")]
    #[case("PageDown")]
    #[case("Tab")]
    #[case("Del")]
    #[case("Insert")]
    #[case("Esc")]
    #[case("Space")]
    #[case("F10")]
    #[case("F11")]
    #[case("F12")]
    #[case("F1")]
    #[case("F2")]
    #[case("F3")]
    #[case("F4")]
    #[case("F5")]
    #[case("F6")]
    #[case("F7")]
    #[case("F8")]
    #[case("F9")]
    fn lone_special_keys_without_brackets_should_deser_as_sequence(#[case] mut input: &str) {
        let input_len = input.chars().count();
        let deserialized = parse_sequence(&mut input).unwrap();
        assert_eq!(deserialized.len(), input_len);

        for (i, mut c) in input.chars().enumerate() {
            let mut modifiers = KeyModifiers::NONE;
            if c.is_uppercase() {
                modifiers |= KeyModifiers::SHIFT;
                c = c.to_ascii_uppercase();
            }

            let key = Key { key: KeyCode::Char(c), modifiers };
            assert_eq!(deserialized[i], key);
        }
    }

    #[rstest]
    #[case("<BS>",        Key { key: KeyCode::Backspace, modifiers: KeyModifiers::NONE })]
    #[case("<Backspace>", Key { key: KeyCode::Backspace, modifiers: KeyModifiers::NONE })]
    #[case("<CR>",        Key { key: KeyCode::Enter,     modifiers: KeyModifiers::NONE })]
    #[case("<Enter>",     Key { key: KeyCode::Enter,     modifiers: KeyModifiers::NONE })]
    #[case("<Left>",      Key { key: KeyCode::Left,      modifiers: KeyModifiers::NONE })]
    #[case("<Right>",     Key { key: KeyCode::Right,     modifiers: KeyModifiers::NONE })]
    #[case("<Up>",        Key { key: KeyCode::Up,        modifiers: KeyModifiers::NONE })]
    #[case("<Down>",      Key { key: KeyCode::Down,      modifiers: KeyModifiers::NONE })]
    #[case("<Home>",      Key { key: KeyCode::Home,      modifiers: KeyModifiers::NONE })]
    #[case("<End>",       Key { key: KeyCode::End,       modifiers: KeyModifiers::NONE })]
    #[case("<PageUp>",    Key { key: KeyCode::PageUp,    modifiers: KeyModifiers::NONE })]
    #[case("<PageDown>",  Key { key: KeyCode::PageDown,  modifiers: KeyModifiers::NONE })]
    #[case("<Tab>",       Key { key: KeyCode::Tab,       modifiers: KeyModifiers::NONE })]
    #[case("<Del>",       Key { key: KeyCode::Delete,    modifiers: KeyModifiers::NONE })]
    #[case("<Insert>",    Key { key: KeyCode::Insert,    modifiers: KeyModifiers::NONE })]
    #[case("<Esc>",       Key { key: KeyCode::Esc,       modifiers: KeyModifiers::NONE })]
    #[case("<Space>",     Key { key: KeyCode::Char(' '), modifiers: KeyModifiers::NONE })]
    #[case("<F10>",       Key { key: KeyCode::F(10),     modifiers: KeyModifiers::NONE })]
    #[case("<F11>",       Key { key: KeyCode::F(11),     modifiers: KeyModifiers::NONE })]
    #[case("<F12>",       Key { key: KeyCode::F(12),     modifiers: KeyModifiers::NONE })]
    #[case("<F1>",        Key { key: KeyCode::F(1),      modifiers: KeyModifiers::NONE })]
    #[case("<F2>",        Key { key: KeyCode::F(2),      modifiers: KeyModifiers::NONE })]
    #[case("<F3>",        Key { key: KeyCode::F(3),      modifiers: KeyModifiers::NONE })]
    #[case("<F4>",        Key { key: KeyCode::F(4),      modifiers: KeyModifiers::NONE })]
    #[case("<F5>",        Key { key: KeyCode::F(5),      modifiers: KeyModifiers::NONE })]
    #[case("<F6>",        Key { key: KeyCode::F(6),      modifiers: KeyModifiers::NONE })]
    #[case("<F7>",        Key { key: KeyCode::F(7),      modifiers: KeyModifiers::NONE })]
    #[case("<F8>",        Key { key: KeyCode::F(8),      modifiers: KeyModifiers::NONE })]
    #[case("<F9>",        Key { key: KeyCode::F(9),      modifiers: KeyModifiers::NONE })]
    fn lone_special(#[case] mut expected_str: &str, #[case] input: Key) {
        let deserialized = parse_sequence(&mut expected_str).unwrap();
        assert_eq!(deserialized[0], input);
    }
}
