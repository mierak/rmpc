use anyhow::{Context, Result};
use itertools::Itertools;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use strum::Display;

use super::defaults;

#[derive(Debug, Serialize, Deserialize)]
pub struct SymbolsFile {
    pub(super) song: String,
    pub(super) dir: String,
    pub(super) marker: String,
}

#[derive(Debug, Default)]
pub struct SymbolsConfig {
    pub song: &'static str,
    pub dir: &'static str,
    pub marker: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProgressBarConfigFile {
    pub(super) symbols: Vec<String>,
    pub(super) track_colors: Option<(String, String)>,
    pub(super) elapsed_colors: Option<(String, String)>,
    pub(super) thumb_colors: Option<(String, String)>,
}

#[derive(Debug)]
pub struct ProgressBarConfig {
    pub symbols: [&'static str; 3],
    pub track_colors: (Color, Color),
    pub elapsed_colors: (Color, Color),
    pub thumb_colors: (Color, Color),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SongTableColumnFile {
    pub(super) prop: SongProperty,
    pub(super) label: Option<String>,
    pub(super) width_percent: u16,
    pub(super) color: Option<String>,
    pub(super) alignment: Option<Alignment>,
}

#[derive(Debug, Copy, Clone)]
pub struct SongTableColumn {
    pub prop: SongProperty,
    pub label: &'static str,
    pub width_percent: u16,
    pub color: Color,
    pub alignment: Alignment,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UiConfigFile {
    #[serde(default = "defaults::default_false")]
    pub(super) disable_images: bool,
    pub(super) symbols: SymbolsFile,
    pub(super) progress_bar: ProgressBarConfigFile,
    #[serde(default = "defaults::default_column_widths")]
    pub(super) browser_column_widths: Vec<u16>,
    pub(super) background_color: Option<String>,
    pub(super) background_color_modal: Option<String>,
    pub(super) current_song_color: Option<String>,
    pub(super) volume_color: Option<String>,
    pub(super) status_color: Option<String>,
    pub(super) show_song_table_header: bool,
    pub(super) song_table_format: Vec<SongTableColumnFile>,
}

#[derive(Debug)]
pub struct UiConfig {
    pub disable_images: bool,
    pub background_color: Option<Color>,
    pub background_color_modal: Option<Color>,
    pub current_song_color: Color,
    pub column_widths: [u16; 3],
    pub symbols: SymbolsConfig,
    pub volume_color: Color,
    pub status_color: Color,
    pub progress_bar: ProgressBarConfig,
    pub show_song_table_header: bool,
    pub song_table_format: Vec<SongTableColumn>,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, Display)]
pub enum SongProperty {
    Duration,
    Filename,
    Artist,
    AlbumArtist,
    Title,
    Album,
    Date,
    Genre,
    Comment,
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
    Rgb(u8, u8, u8),
    Indexed(u8),
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum Alignment {
    Left,
    Right,
    Center,
}

impl From<Alignment> for ratatui::layout::Alignment {
    fn from(value: Alignment) -> Self {
        match value {
            Alignment::Left => Self::Left,
            Alignment::Right => Self::Right,
            Alignment::Center => Self::Center,
        }
    }
}

impl Default for UiConfigFile {
    fn default() -> Self {
        Self {
            disable_images: false,
            background_color: Some("black".to_string()),
            background_color_modal: None,
            current_song_color: Some("blue".to_string()),
            browser_column_widths: vec![20, 38, 42],
            volume_color: Some("blue".to_string()),
            status_color: Some("yellow".to_string()),
            progress_bar: ProgressBarConfigFile {
                symbols: vec!["█".to_owned(), "".to_owned(), "█".to_owned()],
                track_colors: Some(("black".to_string(), "black".to_string())),
                elapsed_colors: Some(("blue".to_string(), "black".to_string())),
                thumb_colors: Some(("blue".to_string(), "black".to_string())),
            },
            symbols: SymbolsFile {
                song: "🎵".to_owned(),
                dir: "📁".to_owned(),
                marker: "".to_owned(),
            },
            show_song_table_header: true,
            song_table_format: vec![
                SongTableColumnFile {
                    prop: SongProperty::Artist,
                    label: None,
                    width_percent: 20,
                    color: None,
                    alignment: None,
                },
                SongTableColumnFile {
                    prop: SongProperty::Title,
                    label: None,
                    width_percent: 35,
                    color: None,
                    alignment: None,
                },
                SongTableColumnFile {
                    prop: SongProperty::Album,
                    label: None,
                    width_percent: 30,
                    color: Some("white".to_string()),
                    alignment: None,
                },
                SongTableColumnFile {
                    prop: SongProperty::Duration,
                    label: None,
                    width_percent: 15,
                    color: None,
                    alignment: Some(Alignment::Right),
                },
            ],
        }
    }
}

impl TryFrom<&[u8]> for crate::config::ConfigColor {
    type Error = anyhow::Error;

    fn try_from(input: &[u8]) -> Result<Self, Self::Error> {
        match input {
            b"reset" => Ok(Self::Reset),
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
                let r = u8::from_str_radix(
                    std::str::from_utf8(&s[1..3]).context("Failed to get str for red color value")?,
                    16,
                )
                .context("Failed to parse red color value")?;
                let g = u8::from_str_radix(
                    std::str::from_utf8(&s[3..5]).context("Failed to get str for green color value")?,
                    16,
                )
                .context("Failed to parse green color value")?;
                let b = u8::from_str_radix(
                    std::str::from_utf8(&s[5..7]).context("Failed to get str for blue color value")?,
                    16,
                )
                .context("Failed to parse blue color value")?;

                Ok(Self::Rgb(r, g, b))
            }
            s if s.starts_with(b"rgb(") => {
                let mut colors =
                    std::str::from_utf8(s.strip_prefix(b"rgb(").context("")?.strip_suffix(b")").context("")?)?
                        .splitn(3, ',');
                let r = colors.next().context("")?.parse::<u8>().context("")?;
                let g = colors.next().context("")?.parse::<u8>().context("")?;
                let b = colors.next().context("")?.parse::<u8>().context("")?;
                Ok(Self::Rgb(r, g, b))
            }
            s => {
                if let Ok(v) = std::str::from_utf8(s)?.parse::<u8>() {
                    Ok(Self::Indexed(v))
                } else {
                    Err(anyhow::anyhow!("Invalid color format '{s:?}'"))
                }
            }
        }
    }
}

impl From<crate::config::ConfigColor> for Color {
    fn from(value: crate::config::ConfigColor) -> Self {
        match value {
            crate::config::ConfigColor::Reset => Color::Reset,
            crate::config::ConfigColor::Black => Color::Black,
            crate::config::ConfigColor::Red => Color::Red,
            crate::config::ConfigColor::Green => Color::Green,
            crate::config::ConfigColor::Yellow => Color::Yellow,
            crate::config::ConfigColor::Blue => Color::Blue,
            crate::config::ConfigColor::Magenta => Color::Magenta,
            crate::config::ConfigColor::Cyan => Color::Cyan,
            crate::config::ConfigColor::Gray => Color::Gray,
            crate::config::ConfigColor::DarkGray => Color::DarkGray,
            crate::config::ConfigColor::LightRed => Color::LightRed,
            crate::config::ConfigColor::LightGreen => Color::LightGreen,
            crate::config::ConfigColor::LightYellow => Color::LightYellow,
            crate::config::ConfigColor::LightBlue => Color::LightBlue,
            crate::config::ConfigColor::LightMagenta => Color::LightMagenta,
            crate::config::ConfigColor::LightCyan => Color::LightCyan,
            crate::config::ConfigColor::White => Color::White,
            crate::config::ConfigColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
            crate::config::ConfigColor::Indexed(v) => Color::Indexed(v),
        }
    }
}

impl TryFrom<UiConfigFile> for UiConfig {
    type Error = anyhow::Error;

    fn try_from(mut value: UiConfigFile) -> Result<Self, Self::Error> {
        let elapsed = std::mem::take(&mut value.progress_bar.symbols[0]);
        let thumb = std::mem::take(&mut value.progress_bar.symbols[1]);
        let track = std::mem::take(&mut value.progress_bar.symbols[2]);
        let progress_bar_color_track = value.progress_bar.track_colors.map_or_else(
            || Ok((Color::Black, Color::Black)),
            |v: (String, String)| -> Result<(Color, Color)> {
                let r = (v.0.as_bytes().try_into(), v.1.as_bytes().try_into());
                let r: (ConfigColor, ConfigColor) = (r.0?, r.1?);
                let r = (r.0.into(), r.1.into());
                Ok(r)
            },
        )?;
        let progress_bar_color_elapsed = value.progress_bar.elapsed_colors.map_or_else(
            || Ok((Color::Blue, Color::Black)),
            |v: (String, String)| -> Result<(Color, Color)> {
                let r = (v.0.as_bytes().try_into(), v.1.as_bytes().try_into());
                let r: (ConfigColor, ConfigColor) = (r.0?, r.1?);
                let r = (r.0.into(), r.1.into());
                Ok(r)
            },
        )?;
        let progress_bar_color_thumb = value.progress_bar.thumb_colors.map_or_else(
            || Ok((Color::Blue, Color::Black)),
            |v: (String, String)| -> Result<(Color, Color)> {
                let r = (v.0.as_bytes().try_into(), v.1.as_bytes().try_into());
                let r: (ConfigColor, ConfigColor) = (r.0?, r.1?);
                let r = (r.0.into(), r.1.into());
                Ok(r)
            },
        )?;

        let bg_color = value
            .background_color
            .map(|v| TryInto::<ConfigColor>::try_into(v.as_bytes()))
            .transpose()?
            .map(Into::into);
        let modal_bg_color = value
            .background_color_modal
            .map(|v| TryInto::<ConfigColor>::try_into(v.as_bytes()))
            .transpose()?
            .map(Into::<Color>::into)
            .or(bg_color);

        if value.song_table_format.iter().map(|v| v.width_percent).sum::<u16>() > 100 {
            anyhow::bail!("Song table format width percent sum is greater than 100");
        }

        Ok(Self {
            disable_images: value.disable_images,
            background_color: bg_color,
            background_color_modal: modal_bg_color,
            current_song_color: value
                .current_song_color
                .map(|v| TryInto::<ConfigColor>::try_into(v.as_bytes()))
                .transpose()?
                .map_or_else(|| Color::Blue, Into::into),
            symbols: value.symbols.into(),
            column_widths: [
                value.browser_column_widths[0],
                value.browser_column_widths[1],
                value.browser_column_widths[2],
            ],
            volume_color: value
                .volume_color
                .map(|v| TryInto::<ConfigColor>::try_into(v.as_bytes()))
                .transpose()?
                .map_or_else(|| Color::Blue, Into::into),
            status_color: value
                .status_color
                .map(|v| TryInto::<ConfigColor>::try_into(v.as_bytes()))
                .transpose()?
                .map_or_else(|| Color::Yellow, Into::into),
            progress_bar: ProgressBarConfig {
                symbols: [
                    Box::leak(Box::new(elapsed)),
                    Box::leak(Box::new(thumb)),
                    Box::leak(Box::new(track)),
                ],
                elapsed_colors: progress_bar_color_elapsed,
                thumb_colors: progress_bar_color_thumb,
                track_colors: progress_bar_color_track,
            },
            show_song_table_header: value.show_song_table_header,
            song_table_format: value
                .song_table_format
                .into_iter()
                .map(|v| -> Result<_> {
                    Ok(SongTableColumn {
                        prop: v.prop,
                        label: Box::leak(Box::new(v.label.unwrap_or_else(|| v.prop.to_string()))),
                        width_percent: v.width_percent,
                        alignment: v.alignment.unwrap_or(Alignment::Left),
                        color: v
                            .color
                            .map(|v| TryInto::<ConfigColor>::try_into(v.as_bytes()))
                            .transpose()?
                            .map_or_else(|| Color::White, Into::into),
                    })
                })
                .try_collect()?,
        })
    }
}

impl From<SymbolsFile> for SymbolsConfig {
    fn from(value: SymbolsFile) -> Self {
        Self {
            song: Box::leak(Box::new(value.song)),
            dir: Box::leak(Box::new(value.dir)),
            marker: Box::leak(Box::new(value.marker)),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::config::ConfigColor;

    #[test]
    fn hex_value() {
        let input: &[u8] = b"#ff00ff";
        let result = ConfigColor::try_from(input).unwrap();
        assert_eq!(result, ConfigColor::Rgb(255, 0, 255));
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
}
