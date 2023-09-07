use std::{collections::HashMap, path::PathBuf};

use clap::Parser;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use tracing::Level;

use crate::ui::{
    screens::{
        albums::AlbumsActions, artists::ArtistsActions, directories::DirectoriesActions, logs::LogsActions,
        queue::QueueuActions,
    },
    GlobalAction,
};

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long, default_value = "127.0.0.1:6600")]
    pub mpd_address: String,
    #[arg(short, long, value_name = "FILE", default_value = get_default_config_path().into_os_string())]
    pub config: PathBuf,
    #[arg(short, long, default_value_t = Level::DEBUG)]
    pub log: Level,
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Key {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFile {
    pub address: String,
    pub keybinds: KeyConfigFile,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyConfigFile {
    pub global: HashMap<GlobalAction, Key>,
    pub albums: HashMap<AlbumsActions, Key>,
    pub artists: HashMap<ArtistsActions, Key>,
    pub directories: HashMap<DirectoriesActions, Key>,
    pub logs: HashMap<LogsActions, Key>,
    pub queue: HashMap<QueueuActions, Key>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            address: String::from("127.0.0.1:6600"),
            keybinds: KeyConfigFile::default(),
        }
    }
}

impl Default for KeyConfigFile {
    #[rustfmt::skip]
    fn default() -> Self {
        use AlbumsActions as Al;
        use ArtistsActions as Ar;
        use DirectoriesActions as D;
        use GlobalAction as G;
        use KeyCode as K;
        use KeyModifiers as M;
        use LogsActions as L;
        use QueueuActions as Q;
        Self {
            global: HashMap::from([
                (G::NextTrack,     Key { key: K::Char('n'), modifiers: M::NONE }),
                (G::PreviousTrack, Key { key: K::Char('p'), modifiers: M::NONE }),
                (G::Stop,          Key { key: K::Char('s'), modifiers: M::NONE }),
                (G::ToggleRepeat,  Key { key: K::Char('z'), modifiers: M::NONE }),
                (G::ToggleRandom,  Key { key: K::Char('x'), modifiers: M::NONE }),
                (G::ToggleSingle,  Key { key: K::Char('c'), modifiers: M::NONE }),
                (G::SeekForward,   Key { key: K::Char('f'), modifiers: M::NONE }),
                (G::SeekBack,      Key { key: K::Char('b'), modifiers: M::NONE }),
                (G::VolumeDown,    Key { key: K::Char(','), modifiers: M::NONE }),
                (G::VolumeUp,      Key { key: K::Char('.'), modifiers: M::NONE }),
                (G::NextTab,       Key { key: K::Right,     modifiers: M::NONE }),
                (G::PreviousTab,   Key { key: K::Left,      modifiers: M::NONE }),
                (G::ToggleConsume, Key { key: K::Char('v'), modifiers: M::NONE }),
            ]),
            albums: HashMap::from([
                (Al::Down,         Key { key: K::Char('j'), modifiers: M::NONE }),
                (Al::Up,           Key { key: K::Char('k'), modifiers: M::NONE }),
                (Al::DownHalf,     Key { key: K::Char('d'), modifiers: M::CONTROL }),
                (Al::UpHalf,       Key { key: K::Char('u'), modifiers: M::CONTROL }),
                (Al::Enter,        Key { key: K::Char('l'), modifiers: M::NONE }),
                (Al::Leave,        Key { key: K::Char('h'), modifiers: M::NONE }),
            ]),
            artists: HashMap::from([
                (Ar::Down,         Key { key: K::Char('j'), modifiers: M::NONE }),
                (Ar::Up,           Key { key: K::Char('k'), modifiers: M::NONE }),
                (Ar::DownHalf,     Key { key: K::Char('d'), modifiers: M::CONTROL }),
                (Ar::UpHalf,       Key { key: K::Char('u'), modifiers: M::CONTROL }),
                (Ar::Enter,        Key { key: K::Char('l'), modifiers: M::NONE }),
                (Ar::Leave,        Key { key: K::Char('h'), modifiers: M::NONE }),
            ]),
            directories: HashMap::from([
                (D::Down,          Key { key: K::Char('j'), modifiers: M::NONE }),
                (D::Up,            Key { key: K::Char('k'), modifiers: M::NONE }),
                (D::DownHalf,      Key { key: K::Char('d'), modifiers: M::CONTROL }),
                (D::UpHalf,        Key { key: K::Char('u'), modifiers: M::CONTROL }),
                (D::Enter,         Key { key: K::Char('l'), modifiers: M::NONE }),
                (D::Leave,         Key { key: K::Char('h'), modifiers: M::NONE }),
                (D::AddAll,        Key { key: K::Char('a'), modifiers: M::NONE }),
            ]),
            logs: HashMap::from([
                (L::Down,          Key { key: K::Char('j'), modifiers: M::NONE }),
                (L::Up,            Key { key: K::Char('k'), modifiers: M::NONE }),
                (L::DownHalf,      Key { key: K::Char('d'), modifiers: M::CONTROL }),
                (L::UpHalf,        Key { key: K::Char('u'), modifiers: M::CONTROL }),
            ]),
            queue: HashMap::from([
                (Q::Down,          Key { key: K::Char('j'), modifiers: M::NONE }),
                (Q::Up,            Key { key: K::Char('k'), modifiers: M::NONE }),
                (Q::DownHalf,      Key { key: K::Char('d'), modifiers: M::CONTROL }),
                (Q::UpHalf,        Key { key: K::Char('u'), modifiers: M::CONTROL }),
                (Q::Top,           Key { key: K::Char('g'), modifiers: M::CONTROL }),
                (Q::Bottom,        Key { key: K::Char('G'), modifiers: M::CONTROL }),
                (Q::TogglePause,   Key { key: K::Char(' '), modifiers: M::NONE }),
                (Q::Delete,        Key { key: K::Char('d'), modifiers: M::NONE }),
                (Q::DeleteAll,     Key { key: K::Char('D'), modifiers: M::NONE }),
                (Q::Play,          Key { key: K::Enter,     modifiers: M::NONE }),
            ]),
        }
    }
}

impl From<ConfigFile> for Config {
    fn from(value: ConfigFile) -> Self {
        Self {
            address: Box::leak(Box::new(value.address)),
            keybinds: KeyConfig {
                global: value.keybinds.global.into_iter().map(|(k, v)| (v, k)).collect(),
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

pub struct Config {
    pub address: &'static str,
    pub keybinds: KeyConfig,
}

pub struct KeyConfig {
    pub global: HashMap<Key, GlobalAction>,
    pub albums: HashMap<Key, AlbumsActions>,
    pub artists: HashMap<Key, ArtistsActions>,
    pub directories: HashMap<Key, DirectoriesActions>,
    pub logs: HashMap<Key, LogsActions>,
    pub queue: HashMap<Key, QueueuActions>,
}
