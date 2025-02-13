use anyhow::{Context, Result, bail};
use bitflags::bitflags;
use bon::Builder;
use ratatui::style::Color as RColor;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

pub(super) trait ToConfigOr {
    fn to_config_or(
        &self,
        default_fg: Option<RColor>,
        default_bg: Option<RColor>,
    ) -> Result<ratatui::style::Style>;
}

pub(super) struct StringColor(pub Option<String>);
impl StringColor {
    pub fn to_color(&self) -> Result<Option<RColor>> {
        let fg: Option<ConfigColor> =
            self.0.as_ref().map(|v| v.as_bytes().try_into()).transpose()?;
        Ok(fg.map(std::convert::Into::into))
    }
}

#[skip_serializing_none]
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Builder)]
pub struct StyleFile {
    pub(super) fg: Option<String>,
    pub(super) bg: Option<String>,
    pub(super) modifiers: Option<Modifiers>,
}

impl std::fmt::Display for Modifiers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.contains(Modifiers::Bold) {
            write!(f, "b")?;
        }
        if self.contains(Modifiers::Dim) {
            write!(f, "d")?;
        }
        if self.contains(Modifiers::Italic) {
            write!(f, "i")?;
        }
        if self.contains(Modifiers::Underlined) {
            write!(f, "u")?;
        }
        if self.contains(Modifiers::Reversed) {
            write!(f, "r")?;
        }
        if self.contains(Modifiers::CrossedOut) {
            write!(f, "c")?;
        }
        Ok(())
    }
}

impl std::fmt::Display for StyleFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Style({},{},{})",
            match self.fg {
                Some(ref fg) => fg.to_owned(),
                None => "none".to_string(),
            },
            match self.bg {
                Some(ref bg) => bg.to_owned(),
                None => "none".to_string(),
            },
            self.modifiers.as_ref().map_or_else(|| "none".to_string(), ToString::to_string)
        )
    }
}

#[allow(clippy::similar_names)]
impl ToConfigOr for StyleFile {
    fn to_config_or(
        &self,
        default_fg: Option<RColor>,
        default_bg: Option<RColor>,
    ) -> Result<ratatui::style::Style> {
        let fg: Option<ConfigColor> =
            self.fg.as_ref().map(|s| s.as_bytes().try_into()).transpose()?;
        let fg: Option<RColor> = fg.map(Into::into).or(default_fg);

        let bg: Option<ConfigColor> =
            self.bg.as_ref().map(|s| s.as_bytes().try_into()).transpose()?;
        let bg: Option<RColor> = bg.map(Into::into).or(default_bg);

        let modifiers =
            self.modifiers.as_ref().map_or(ratatui::style::Modifier::empty(), Into::into);

        let mut result = ratatui::style::Style::default();
        if let Some(fg) = fg {
            result = result.fg(fg);
        }
        if let Some(bg) = bg {
            result = result.bg(bg);
        }

        Ok(result.add_modifier(modifiers))
    }
}

#[allow(clippy::similar_names)]
impl ToConfigOr for Option<StyleFile> {
    fn to_config_or(
        &self,
        default_fg: Option<RColor>,
        default_bg: Option<RColor>,
    ) -> Result<ratatui::style::Style> {
        if let Some(val) = self {
            let fg: Option<ConfigColor> =
                val.fg.as_ref().map(|s| s.as_bytes().try_into()).transpose()?;
            let fg: Option<RColor> = fg.map(Into::into).or(default_fg);

            let bg: Option<ConfigColor> =
                val.bg.as_ref().map(|s| s.as_bytes().try_into()).transpose()?;
            let bg: Option<RColor> = bg.map(Into::into).or(default_bg);

            let modifiers =
                val.modifiers.as_ref().map_or(ratatui::style::Modifier::empty(), Into::into);

            let mut result = ratatui::style::Style::default();
            if let Some(fg) = fg {
                result = result.fg(fg);
            }
            if let Some(bg) = bg {
                result = result.bg(bg);
            }
            Ok(result.add_modifier(modifiers))
        } else {
            let mut result = ratatui::style::Style::default();
            if let Some(fg) = default_fg {
                result = result.fg(fg);
            }
            if let Some(bg) = default_bg {
                result = result.bg(bg);
            }
            Ok(result)
        }
    }
}

impl TryFrom<&[u8]> for crate::config::ConfigColor {
    type Error = anyhow::Error;

    fn try_from(input: &[u8]) -> Result<Self, Self::Error> {
        match input {
            b"reset" => Ok(Self::Reset),
            b"default" => Ok(Self::Reset),
            b"black" => Ok(Self::Black),
            b"red" => Ok(Self::Red),
            b"green" => Ok(Self::Green),
            b"yellow" => Ok(Self::Yellow),
            b"blue" => Ok(Self::Blue),
            b"magenta" => Ok(Self::Magenta),
            b"cyan" => Ok(Self::Cyan),
            b"gray" => Ok(Self::Gray),
            b"dark_gray" => Ok(Self::DarkGray),
            b"light_red" => Ok(Self::LightRed),
            b"light_green" => Ok(Self::LightGreen),
            b"light_yellow" => Ok(Self::LightYellow),
            b"light_blue" => Ok(Self::LightBlue),
            b"light_magenta" => Ok(Self::LightMagenta),
            b"light_cyan" => Ok(Self::LightCyan),
            b"white" => Ok(Self::White),
            s if input.len() == 7 && input.first().is_some_and(|v| v == &b'#') => {
                let res = std::str::from_utf8(s.strip_prefix(b"#").context("")?)?;
                let res = u32::from_str_radix(res, 16).context("")?;
                Ok(Self::Hex(res))
            }
            s if s.starts_with(b"rgb(") => {
                let mut colors = std::str::from_utf8(
                    s.strip_prefix(b"rgb(").context("")?.strip_suffix(b")").context("")?,
                )?
                .splitn(3, ',');
                let r = colors.next().context("")?.parse::<u8>().context("")?;
                let g = colors.next().context("")?.parse::<u8>().context("")?;
                let b = colors.next().context("")?.parse::<u8>().context("")?;
                Ok(Self::Rgb(r, g, b))
            }
            s => {
                if let Ok(s) = std::str::from_utf8(s) {
                    if let Ok(v) = s.parse::<u8>() {
                        Ok(Self::Indexed(v))
                    } else {
                        bail!("Invalid color format '{s}'")
                    }
                } else {
                    bail!("Invalid color format '{s:?}'")
                }
            }
        }
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
    pub struct Modifiers: u16 {
        const Bold       = 0b0000_0000_0001;
        const Dim        = 0b0000_0000_0010;
        const Italic     = 0b0000_0000_0100;
        const Underlined = 0b0000_0000_1000;
        const Reversed   = 0b0000_0100_0000;
        const CrossedOut = 0b0001_0000_0000;
    }
}

impl From<Modifiers> for ratatui::style::Modifier {
    fn from(value: Modifiers) -> Self {
        (&value).into()
    }
}

impl From<&Modifiers> for ratatui::style::Modifier {
    fn from(value: &Modifiers) -> Self {
        Self::from_bits_retain(value.bits())
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum ConfigColor {
    Reset,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    White,
    Hex(u32),
    Rgb(u8, u8, u8),
    Indexed(u8),
}

impl From<crate::config::ConfigColor> for RColor {
    fn from(value: crate::config::ConfigColor) -> Self {
        use crate::config::ConfigColor as CColor;
        match value {
            CColor::Reset => RColor::Reset,
            CColor::Black => RColor::Black,
            CColor::Red => RColor::Red,
            CColor::Green => RColor::Green,
            CColor::Yellow => RColor::Yellow,
            CColor::Blue => RColor::Blue,
            CColor::Magenta => RColor::Magenta,
            CColor::Cyan => RColor::Cyan,
            CColor::Gray => RColor::Gray,
            CColor::DarkGray => RColor::DarkGray,
            CColor::LightRed => RColor::LightRed,
            CColor::LightGreen => RColor::LightGreen,
            CColor::LightYellow => RColor::LightYellow,
            CColor::LightBlue => RColor::LightBlue,
            CColor::LightMagenta => RColor::LightMagenta,
            CColor::LightCyan => RColor::LightCyan,
            CColor::White => RColor::White,
            CColor::Rgb(r, g, b) => RColor::Rgb(r, g, b),
            CColor::Hex(v) => RColor::from_u32(v),
            CColor::Indexed(v) => RColor::Indexed(v),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use ratatui::style::Modifier as RM;
    use test_case::test_case;

    use crate::config::{ConfigColor, theme::style::Modifiers};

    #[test]
    #[rustfmt::skip]
    fn string_values() {
        assert_eq!(ConfigColor::try_from("reset".as_bytes()).unwrap(), ConfigColor::Reset);
        assert_eq!(ConfigColor::try_from("default".as_bytes()).unwrap(), ConfigColor::Reset);
        assert_eq!(ConfigColor::try_from("black".as_bytes()).unwrap(), ConfigColor::Black);
        assert_eq!(ConfigColor::try_from("red".as_bytes()).unwrap(), ConfigColor::Red);
        assert_eq!(ConfigColor::try_from("green".as_bytes()).unwrap(), ConfigColor::Green);
        assert_eq!(ConfigColor::try_from("yellow".as_bytes()).unwrap(), ConfigColor::Yellow);
        assert_eq!(ConfigColor::try_from("blue".as_bytes()).unwrap(), ConfigColor::Blue);
        assert_eq!(ConfigColor::try_from("magenta".as_bytes()).unwrap(), ConfigColor::Magenta);
        assert_eq!(ConfigColor::try_from("cyan".as_bytes()).unwrap(), ConfigColor::Cyan);
        assert_eq!(ConfigColor::try_from("gray".as_bytes()).unwrap(), ConfigColor::Gray);
        assert_eq!(ConfigColor::try_from("dark_gray".as_bytes()).unwrap(), ConfigColor::DarkGray);
        assert_eq!(ConfigColor::try_from("light_red".as_bytes()).unwrap(), ConfigColor::LightRed);
        assert_eq!(ConfigColor::try_from("light_green".as_bytes()).unwrap(), ConfigColor::LightGreen);
        assert_eq!(ConfigColor::try_from("light_yellow".as_bytes()).unwrap(), ConfigColor::LightYellow);
        assert_eq!(ConfigColor::try_from("light_blue".as_bytes()).unwrap(), ConfigColor::LightBlue);
        assert_eq!(ConfigColor::try_from("light_magenta".as_bytes()).unwrap(), ConfigColor::LightMagenta);
        assert_eq!(ConfigColor::try_from("light_cyan".as_bytes()).unwrap(), ConfigColor::LightCyan);
        assert_eq!(ConfigColor::try_from("white".as_bytes()).unwrap(), ConfigColor::White);
    }

    #[test]
    fn hex_value() {
        let input: &[u8] = b"#ff00ff";
        let result = ConfigColor::try_from(input).unwrap();
        assert_eq!(result, ConfigColor::Hex(16_711_935));
    }

    #[test]
    fn invalid_hex_value() {
        let input: &[u8] = b"#ff00f";
        let result = ConfigColor::try_from(input);
        assert!(result.is_err());
    }

    #[test]
    fn rgb_value() {
        let input: &[u8] = b"rgb(255,0,255)";
        let result = ConfigColor::try_from(input).unwrap();
        assert_eq!(result, ConfigColor::Rgb(255, 0, 255));
    }

    #[test]
    fn invalid_rgb_value() {
        let input: &[u8] = b"rgb(255,0,256)";
        let result = ConfigColor::try_from(input);
        assert!(result.is_err());
    }

    #[test]
    fn indexed_value() {
        let input: &[u8] = b"255";
        let result = ConfigColor::try_from(input).unwrap();
        assert_eq!(result, ConfigColor::Indexed(255));
    }

    #[test]
    fn invalid_indexed_value() {
        let input: &[u8] = b"256";
        let result = ConfigColor::try_from(input);
        assert!(result.is_err());
    }

    #[test_case(Modifiers::Bold,       RM::BOLD; "bold")]
    #[test_case(Modifiers::Dim,        RM::DIM; "dim")]
    #[test_case(Modifiers::Italic,     RM::ITALIC; "italic")]
    #[test_case(Modifiers::Underlined, RM::UNDERLINED; "underlined")]
    #[test_case(Modifiers::Reversed,   RM::REVERSED; "reversed")]
    #[test_case(Modifiers::CrossedOut, RM::CROSSED_OUT; "crossed out")]
    fn single_modifiers(input: Modifiers, expected: RM) {
        let result: RM = input.into();

        assert_eq!(result, expected);
    }

    #[test]
    fn modifiers_group1() {
        let result: RM = (Modifiers::Bold | Modifiers::Dim | Modifiers::Italic).into();

        assert_eq!(result, RM::BOLD | RM::DIM | RM::ITALIC);
    }

    #[test]
    fn modifiers_group2() {
        let result: RM =
            (Modifiers::Underlined | Modifiers::Reversed | Modifiers::CrossedOut).into();

        assert_eq!(result, RM::UNDERLINED | RM::REVERSED | RM::CROSSED_OUT);
    }
}
