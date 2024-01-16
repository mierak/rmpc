use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use strum::Display;
use tracing::Level;

use crate::ui::{
    screens::{
        albums::AlbumsActions, artists::ArtistsActions, directories::DirectoriesActions, logs::LogsActions,
        playlists::PlaylistsActions, queue::QueueActions, CommonAction,
    },
    GlobalAction,
};

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long, value_name = "FILE", default_value = get_default_config_path().into_os_string())]
    pub config: PathBuf,
    #[arg(short, long, default_value_t = Level::DEBUG)]
    pub log: Level,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Clone, Debug, PartialEq)]
pub enum Command {
    /// Prints the default config. Can be used to bootstrap your config file.
    Config,
}

fn get_default_config_path() -> PathBuf {
    let mut path = PathBuf::new();
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        path.push(dir);
    } else if let Ok(home) = std::env::var("HOME") {
        path.push(home);
        path.push(".config");
    } else {
        return path;
    }
    path.push("mpdox");
    path.push("config.ron");
    return path;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SymbolsFile {
    song: String,
    dir: String,
    marker: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Key {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ProgressBarConfigFile {
    symbols: Vec<String>,
    track_colors: Option<(String, String)>,
    elapsed_colors: Option<(String, String)>,
    thumb_colors: Option<(String, String)>,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct SongTableColumnFile {
    prop: SongProperty,
    label: Option<String>,
    width_percent: u16,
    color: Option<String>,
    alignment: Option<Alignment>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UiConfigFile {
    symbols: SymbolsFile,
    progress_bar: ProgressBarConfigFile,
    #[serde(default = "defaults::default_column_widths")]
    column_widths: Vec<u16>,
    background_color: Option<String>,
    background_color_modal: Option<String>,
    volume_color: Option<String>,
    status_color: Option<String>,
    show_song_table_header: bool,
    song_table_format: Vec<SongTableColumnFile>,
}

impl Default for UiConfigFile {
    fn default() -> Self {
        Self {
            background_color: Some("black".to_string()),
            background_color_modal: None,
            column_widths: vec![20, 38, 42],
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFile {
    address: String,
    #[serde(default = "defaults::default_volume_step")]
    volume_step: u8,
    #[serde(default = "defaults::default_false")]
    disable_images: bool,
    #[serde(default = "defaults::default_progress_update_interval_ms")]
    status_update_interval_ms: Option<u64>,
    keybinds: KeyConfigFile,
    ui: Option<UiConfigFile>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyConfigFile {
    pub global: HashMap<GlobalAction, Vec<Key>>,
    pub navigation: HashMap<CommonAction, Vec<Key>>,
    pub albums: HashMap<AlbumsActions, Vec<Key>>,
    pub artists: HashMap<ArtistsActions, Vec<Key>>,
    pub directories: HashMap<DirectoriesActions, Vec<Key>>,
    pub playlists: HashMap<PlaylistsActions, Vec<Key>>,
    pub logs: HashMap<LogsActions, Vec<Key>>,
    pub queue: HashMap<QueueActions, Vec<Key>>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            address: String::from("127.0.0.1:6600"),
            keybinds: KeyConfigFile::default(),
            volume_step: 5,
            disable_images: false,
            status_update_interval_ms: Some(1000),
            ui: Some(UiConfigFile::default()),
        }
    }
}

mod defaults {
    pub fn default_column_widths() -> Vec<u16> {
        vec![20, 38, 42]
    }

    pub fn default_false() -> bool {
        false
    }

    pub fn default_volume_step() -> u8 {
        5
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn default_progress_update_interval_ms() -> Option<u64> {
        Some(1000)
    }
}

impl Default for KeyConfigFile {
    #[rustfmt::skip]
    #[allow(unused_imports)]
    fn default() -> Self {
        use GlobalAction as G;
        use CommonAction as C;
        use AlbumsActions as Al;
        use ArtistsActions as Ar;
        use DirectoriesActions as D;
        use PlaylistsActions as P;
        use KeyCode as K;
        use KeyModifiers as M;
        use LogsActions as L;
        use QueueActions as Q;
        Self {
            global: HashMap::from([
                (G::Quit,             vec![Key { key: K::Char('q'), modifiers: M::NONE }]),
                (G::NextTrack,        vec![Key { key: K::Char('>'), modifiers: M::NONE }]),
                (G::PreviousTrack,    vec![Key { key: K::Char('<'), modifiers: M::NONE }]),
                (G::Stop,             vec![Key { key: K::Char('s'), modifiers: M::NONE }]),
                (G::ToggleRepeat,     vec![Key { key: K::Char('z'), modifiers: M::NONE }]),
                (G::ToggleRandom,     vec![Key { key: K::Char('x'), modifiers: M::NONE }]),
                (G::ToggleSingle,     vec![Key { key: K::Char('c'), modifiers: M::NONE }]),
                (G::TogglePause,      vec![Key { key: K::Char('p'), modifiers: M::NONE }]),
                (G::SeekForward,      vec![Key { key: K::Char('f'), modifiers: M::NONE }]),
                (G::SeekBack,         vec![Key { key: K::Char('b'), modifiers: M::NONE }]),
                (G::VolumeDown,       vec![Key { key: K::Char(','), modifiers: M::NONE }]),
                (G::VolumeUp,         vec![Key { key: K::Char('.'), modifiers: M::NONE }]),
                (G::NextTab,          vec![Key { key: K::Right,     modifiers: M::NONE }]),
                (G::PreviousTab,      vec![Key { key: K::Left,      modifiers: M::NONE }]),
                (G::ToggleConsume,    vec![Key { key: K::Char('v'), modifiers: M::NONE }]),
            ]),
            navigation: HashMap::from([
                (C::Up,               vec![Key { key: K::Char('k'), modifiers: M::NONE }]),
                (C::Down,             vec![Key { key: K::Char('j'), modifiers: M::NONE }]),
                (C::MoveUp,           vec![Key { key: K::Char('K'), modifiers: M::SHIFT }]),
                (C::MoveDown,         vec![Key { key: K::Char('J'), modifiers: M::SHIFT }]),
                (C::Right,            vec![Key { key: K::Char('l'), modifiers: M::NONE }]),
                (C::Left,             vec![Key { key: K::Char('h'), modifiers: M::NONE }]),
                (C::DownHalf,         vec![Key { key: K::Char('d'), modifiers: M::CONTROL }]),
                (C::UpHalf,           vec![Key { key: K::Char('u'), modifiers: M::CONTROL }]),
                (C::Bottom,           vec![Key { key: K::Char('G'), modifiers: M::SHIFT }]),
                (C::Top,              vec![Key { key: K::Char('g'), modifiers: M::NONE }]),
                (C::EnterSearch,      vec![Key { key: K::Char('/'), modifiers: M::NONE }]),
                (C::NextResult,       vec![Key { key: K::Char('n'), modifiers: M::CONTROL }]),
                (C::PreviousResult,   vec![Key { key: K::Char('N'), modifiers: M::SHIFT }]),
                (C::Select,           vec![Key { key: K::Char(' '), modifiers: M::NONE }]),
                (C::Add,              vec![Key { key: K::Char('a'), modifiers: M::NONE }]),
                (C::Delete,           vec![Key { key: K::Char('D'), modifiers: M::SHIFT }]),
                (C::Rename,           vec![Key { key: K::Char('r'), modifiers: M::NONE }]),
                (C::Close,            vec![Key { key: K::Char('c'), modifiers: M::CONTROL }, Key { key: K::Esc, modifiers: M::NONE }]),
                (C::Confirm,          vec![Key { key: K::Enter,     modifiers: M::NONE }]),
                (C::FocusInput,       vec![Key { key: K::Char('i'), modifiers: M::NONE }]),
            ]),
            albums: HashMap::from([
            ]),
            artists: HashMap::from([
            ]),
            directories: HashMap::from([
            ]),
            playlists: HashMap::from([
            ]),
            logs: HashMap::from([
                (L::Clear,            vec![Key { key: K::Char('D'), modifiers: M::SHIFT }]),
            ]),
            queue: HashMap::from([
                (Q::Delete,           vec![Key { key: K::Char('d'), modifiers: M::NONE }]),
                (Q::DeleteAll,        vec![Key { key: K::Char('D'), modifiers: M::SHIFT }]),
                (Q::Play,             vec![Key { key: K::Enter,     modifiers: M::NONE }]),
                (Q::Save,             vec![Key { key: K::Char('s'), modifiers: M::CONTROL }]),
                (Q::AddToPlaylist,    vec![Key { key: K::Char('a'), modifiers: M::NONE }]),
            ]),
        }
    }
}

impl TryFrom<ConfigFile> for Config {
    type Error = anyhow::Error;

    fn try_from(value: ConfigFile) -> Result<Self, Self::Error> {
        Ok(Self {
            ui: value.ui.unwrap_or_default().try_into()?,
            address: Box::leak(Box::new(value.address)),
            volume_step: value.volume_step,
            disable_images: value.disable_images,
            status_update_interval_ms: value.status_update_interval_ms.map(|v| v.max(100)),
            keybinds: value.keybinds.into(),
        })
    }
}

fn invert_map<T: Copy, V: std::hash::Hash + std::cmp::Eq>(v: HashMap<T, Vec<V>>) -> HashMap<V, T> {
    v.into_iter()
        .flat_map(|(k, v)| v.into_iter().map(move |v| (v, k)))
        .collect()
}

impl From<KeyConfigFile> for KeyConfig {
    fn from(value: KeyConfigFile) -> Self {
        KeyConfig {
            global: invert_map(value.global),
            navigation: invert_map(value.navigation),
            albums: invert_map(value.albums),
            artists: invert_map(value.artists),
            directories: invert_map(value.directories),
            playlists: invert_map(value.playlists),
            logs: invert_map(value.logs),
            queue: invert_map(value.queue),
        }
    }
}

impl From<KeyEvent> for Key {
    fn from(value: KeyEvent) -> Self {
        Self {
            key: value.code,
            modifiers: value.modifiers,
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
            background_color: bg_color,
            background_color_modal: modal_bg_color,
            symbols: value.symbols.into(),
            column_widths: [value.column_widths[0], value.column_widths[1], value.column_widths[2]],
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
                .map(|v| SongTableColumn {
                    prop: v.prop,
                    label: Box::leak(Box::new(v.label.unwrap_or_else(|| v.prop.to_string()))),
                    width_percent: v.width_percent,
                    alignment: v.alignment.unwrap_or(Alignment::Left),
                    color: v
                        .color
                        .map(|v| TryInto::<ConfigColor>::try_into(v.as_bytes()))
                        .transpose()
                        .unwrap_or_default()
                        .map_or_else(|| Color::White, Into::into),
                })
                .collect(),
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

#[derive(Debug)]
pub struct Config {
    pub address: &'static str,
    pub volume_step: u8,
    pub keybinds: KeyConfig,
    pub disable_images: bool,
    pub status_update_interval_ms: Option<u64>,
    pub ui: UiConfig,
}

#[derive(Debug)]
pub struct KeyConfig {
    pub global: HashMap<Key, GlobalAction>,
    pub navigation: HashMap<Key, CommonAction>,
    pub albums: HashMap<Key, AlbumsActions>,
    pub artists: HashMap<Key, ArtistsActions>,
    pub directories: HashMap<Key, DirectoriesActions>,
    pub playlists: HashMap<Key, PlaylistsActions>,
    pub logs: HashMap<Key, LogsActions>,
    pub queue: HashMap<Key, QueueActions>,
}

#[derive(Debug, Default)]
pub struct SymbolsConfig {
    pub song: &'static str,
    pub dir: &'static str,
    pub marker: &'static str,
}

#[derive(Debug)]
pub struct ProgressBarConfig {
    pub symbols: [&'static str; 3],
    pub track_colors: (Color, Color),
    pub elapsed_colors: (Color, Color),
    pub thumb_colors: (Color, Color),
}

#[derive(Debug)]
pub struct UiConfig {
    pub background_color: Option<Color>,
    pub background_color_modal: Option<Color>,
    pub column_widths: [u16; 3],
    pub symbols: SymbolsConfig,
    pub volume_color: Color,
    pub status_color: Color,
    pub progress_bar: ProgressBarConfig,
    pub show_song_table_header: bool,
    pub song_table_format: Vec<SongTableColumn>,
}

#[derive(Debug, Copy, Clone)]
pub struct SongTableColumn {
    pub prop: SongProperty,
    pub label: &'static str,
    pub width_percent: u16,
    pub color: Color,
    pub alignment: Alignment,
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
