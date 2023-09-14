use std::{collections::HashMap, path::PathBuf};

use clap::{Parser, Subcommand};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use tracing::Level;

use crate::ui::{
    screens::{
        albums::AlbumsActions, artists::ArtistsActions, directories::DirectoriesActions, logs::LogsActions,
        queue::QueueActions, CommonAction,
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
    #[serde(default = "defaults::default_false")]
    disable_images: bool,
    keybinds: KeyConfigFile,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyConfigFile {
    pub global: HashMap<GlobalAction, Key>,
    pub navigation: HashMap<CommonAction, Key>,
    pub albums: HashMap<AlbumsActions, Key>,
    pub artists: HashMap<ArtistsActions, Key>,
    pub directories: HashMap<DirectoriesActions, Key>,
    pub logs: HashMap<LogsActions, Key>,
    pub queue: HashMap<QueueActions, Key>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            address: String::from("127.0.0.1:6600"),
            keybinds: KeyConfigFile::default(),
            disable_images: false,
            column_widths: vec![20, 38, 42],
            symbols: SymbolsFile {
                progress_bar: vec!["█".to_owned(), "".to_owned(), "█".to_owned()],
                song: " 🎵".to_owned(),
                dir: " 📁".to_owned(),
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
        use KeyCode as K;
        use KeyModifiers as M;
        use LogsActions as L;
        use QueueActions as Q;
        Self {
            global: HashMap::from([
                (G::NextTrack,        Key { key: K::Char('n'), modifiers: M::NONE }),
                (G::PreviousTrack,    Key { key: K::Char('p'), modifiers: M::NONE }),
                (G::Stop,             Key { key: K::Char('s'), modifiers: M::NONE }),
                (G::ToggleRepeat,     Key { key: K::Char('z'), modifiers: M::NONE }),
                (G::ToggleRandom,     Key { key: K::Char('x'), modifiers: M::NONE }),
                (G::ToggleSingle,     Key { key: K::Char('c'), modifiers: M::NONE }),
                (G::SeekForward,      Key { key: K::Char('f'), modifiers: M::NONE }),
                (G::SeekBack,         Key { key: K::Char('b'), modifiers: M::NONE }),
                (G::VolumeDown,       Key { key: K::Char(','), modifiers: M::NONE }),
                (G::VolumeUp,         Key { key: K::Char('.'), modifiers: M::NONE }),
                (G::NextTab,          Key { key: K::Right,     modifiers: M::NONE }),
                (G::PreviousTab,      Key { key: K::Left,      modifiers: M::NONE }),
                (G::ToggleConsume,    Key { key: K::Char('v'), modifiers: M::NONE }),
            ]),
            navigation: HashMap::from([
                (C::Down,             Key { key: K::Char('j'), modifiers: M::NONE }),
                (C::Up,               Key { key: K::Char('k'), modifiers: M::NONE }),
                (C::Right,            Key { key: K::Char('l'), modifiers: M::NONE }),
                (C::Left,             Key { key: K::Char('h'), modifiers: M::NONE }),
                (C::DownHalf,         Key { key: K::Char('d'), modifiers: M::CONTROL }),
                (C::UpHalf,           Key { key: K::Char('u'), modifiers: M::CONTROL }),
                (C::Bottom,           Key { key: K::Char('G'), modifiers: M::SHIFT }),
                (C::Top,              Key { key: K::Char('g'), modifiers: M::NONE }),
                (C::EnterSearch,      Key { key: K::Char('/'), modifiers: M::NONE }),
                (C::NextResult,       Key { key: K::Char('n'), modifiers: M::CONTROL }),
                (C::PreviousResult,   Key { key: K::Char('p'), modifiers: M::CONTROL }),
            ]),
            albums: HashMap::from([
            ]),
            artists: HashMap::from([
            ]),
            directories: HashMap::from([
                (D::AddAll,           Key { key: K::Char('a'), modifiers: M::NONE }),
            ]),
            logs: HashMap::from([
                (L::Clear,            Key { key: K::Char('D'), modifiers: M::SHIFT }),
            ]),
            queue: HashMap::from([
                (Q::TogglePause,      Key { key: K::Char(' '), modifiers: M::NONE }),
                (Q::Delete,           Key { key: K::Char('d'), modifiers: M::NONE }),
                (Q::DeleteAll,        Key { key: K::Char('D'), modifiers: M::SHIFT }),
                (Q::Play,             Key { key: K::Enter,     modifiers: M::NONE }),
            ]),
        }
    }
}

impl From<ConfigFile> for Config {
    fn from(value: ConfigFile) -> Self {
        Self {
            address: Box::leak(Box::new(value.address)),
            symbols: value.symbols.into(),
            disable_images: value.disable_images,
            column_widths: [value.column_widths[0], value.column_widths[1], value.column_widths[2]],
            keybinds: KeyConfig {
                global: value.keybinds.global.into_iter().map(|(k, v)| (v, k)).collect(),
                navigation: value.keybinds.navigation.into_iter().map(|(k, v)| (v, k)).collect(),
                albums: value.keybinds.albums.into_iter().map(|(k, v)| (v, k)).collect(),
                artists: value.keybinds.artists.into_iter().map(|(k, v)| (v, k)).collect(),
                directories: value.keybinds.directories.into_iter().map(|(k, v)| (v, k)).collect(),
                logs: value.keybinds.logs.into_iter().map(|(k, v)| (v, k)).collect(),
                queue: value.keybinds.queue.into_iter().map(|(k, v)| (v, k)).collect(),
            },
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
        }
    }
}

#[derive(Debug)]
pub struct Config {
    pub address: &'static str,
    pub symbols: SymbolsConfig,
    pub keybinds: KeyConfig,
    pub column_widths: [u16; 3],
    pub disable_images: bool,
}

#[derive(Debug)]
pub struct KeyConfig {
    pub global: HashMap<Key, GlobalAction>,
    pub navigation: HashMap<Key, CommonAction>,
    pub albums: HashMap<Key, AlbumsActions>,
    pub artists: HashMap<Key, ArtistsActions>,
    pub directories: HashMap<Key, DirectoriesActions>,
    pub logs: HashMap<Key, LogsActions>,
    pub queue: HashMap<Key, QueueActions>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SymbolsConfig {
    pub progress_bar: [&'static str; 3],
    pub song: &'static str,
    pub dir: &'static str,
}
