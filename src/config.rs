use std::{collections::HashMap, path::PathBuf};

use clap::{Parser, Subcommand};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
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
    /// Prints the default config.
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
    progress_bar: Vec<String>,
    song: String,
    dir: String,
    marker: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Key {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFile {
    address: String,
    symbols: SymbolsFile,
    #[serde(default = "defaults::default_column_widths")]
    column_widths: Vec<u16>,
    #[serde(default = "defaults::default_volume_step")]
    volume_step: u8,
    #[serde(default = "defaults::default_false")]
    disable_images: bool,
    #[serde(default = "defaults::default_progress_update_interval_ms")]
    status_update_interval_ms: Option<u64>,
    keybinds: KeyConfigFile,
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
            column_widths: vec![20, 38, 42],
            status_update_interval_ms: Some(1000),
            symbols: SymbolsFile {
                progress_bar: vec!["█".to_owned(), "".to_owned(), "█".to_owned()],
                song: "🎵".to_owned(),
                dir: "📁".to_owned(),
                marker: "".to_owned(),
            },
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

impl From<ConfigFile> for Config {
    fn from(value: ConfigFile) -> Self {
        Self {
            address: Box::leak(Box::new(value.address)),
            symbols: value.symbols.into(),
            volume_step: value.volume_step,
            disable_images: value.disable_images,
            column_widths: [value.column_widths[0], value.column_widths[1], value.column_widths[2]],
            status_update_interval_ms: value.status_update_interval_ms.map(|v| v.max(100)),
            keybinds: value.keybinds.into(),
        }
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

impl From<SymbolsFile> for SymbolsConfig {
    fn from(mut value: SymbolsFile) -> Self {
        let elapsed = std::mem::take(&mut value.progress_bar[0]);
        let thumb = std::mem::take(&mut value.progress_bar[1]);
        let track = std::mem::take(&mut value.progress_bar[2]);
        Self {
            progress_bar: [
                Box::leak(Box::new(elapsed)),
                Box::leak(Box::new(thumb)),
                Box::leak(Box::new(track)),
            ],
            song: Box::leak(Box::new(value.song)),
            dir: Box::leak(Box::new(value.dir)),
            marker: Box::leak(Box::new(value.marker)),
        }
    }
}

#[derive(Debug)]
pub struct Config {
    pub address: &'static str,
    pub symbols: SymbolsConfig,
    pub volume_step: u8,
    pub keybinds: KeyConfig,
    pub column_widths: [u16; 3],
    pub disable_images: bool,
    pub status_update_interval_ms: Option<u64>,
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

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SymbolsConfig {
    pub progress_bar: [&'static str; 3],
    pub song: &'static str,
    pub dir: &'static str,
    pub marker: &'static str,
}
