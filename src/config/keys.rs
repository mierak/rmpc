use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

#[cfg(debug_assertions)]
use crate::ui::screens::logs::LogsActions;
use crate::ui::{
    screens::{
        albums::AlbumsActions, artists::ArtistsActions, directories::DirectoriesActions, playlists::PlaylistsActions,
        queue::QueueActions, search::SearchActions, CommonAction,
    },
    GlobalAction,
};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Key {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, PartialEq)]
pub struct KeyConfig {
    pub global: HashMap<Key, GlobalAction>,
    pub navigation: HashMap<Key, CommonAction>,
    pub albums: HashMap<Key, AlbumsActions>,
    pub artists: HashMap<Key, ArtistsActions>,
    pub directories: HashMap<Key, DirectoriesActions>,
    pub playlists: HashMap<Key, PlaylistsActions>,
    pub search: HashMap<Key, SearchActions>,
    #[cfg(debug_assertions)]
    pub logs: HashMap<Key, LogsActions>,
    pub queue: HashMap<Key, QueueActions>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyConfigFile {
    #[serde(default)]
    pub global: HashMap<GlobalAction, Vec<Key>>,
    #[serde(default)]
    pub navigation: HashMap<CommonAction, Vec<Key>>,
    // pub albums: HashMap<AlbumsActions, Vec<Key>>,
    // pub artists: HashMap<ArtistsActions, Vec<Key>>,
    // pub directories: HashMap<DirectoriesActions, Vec<Key>>,
    // pub playlists: HashMap<PlaylistsActions, Vec<Key>>,
    // pub search: HashMap<SearchActions, Vec<Key>>,
    #[cfg(debug_assertions)]
    pub logs: HashMap<LogsActions, Vec<Key>>,
    #[serde(default)]
    pub queue: HashMap<QueueActions, Vec<Key>>,
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
        #[cfg(debug_assertions)]
        use LogsActions as L;
        use QueueActions as Q;
        Self {
            global: HashMap::from([
                (G::Quit,             vec![Key { key: K::Char('q'), modifiers: M::NONE }]),
                (G::NextTrack,        vec![Key { key: K::Char('>'), modifiers: M::SHIFT | M::CONTROL }]),
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
                (G::PreviousTab,      vec![Key { key: K::Left,      modifiers: M::NONE }, Key { key: K::BackTab,  modifiers: M::SHIFT }]),
                (G::NextTab,          vec![Key { key: K::Right,     modifiers: M::NONE }, Key { key: K::Tab,      modifiers: M::NONE }]),
                (G::ToggleConsume,    vec![Key { key: K::Char('v'), modifiers: M::NONE }]),
                (G::QueueTab,         vec![Key { key: K::Char('1'), modifiers: M::NONE }]),
                (G::DirectoriesTab,   vec![Key { key: K::Char('2'), modifiers: M::NONE }]),
                (G::ArtistsTab,       vec![Key { key: K::Char('3'), modifiers: M::NONE }]),
                (G::AlbumsTab,        vec![Key { key: K::Char('4'), modifiers: M::NONE }]),
                (G::PlaylistsTab,     vec![Key { key: K::Char('5'), modifiers: M::NONE }]),
                (G::SearchTab,        vec![Key { key: K::Char('6'), modifiers: M::NONE }]),
            ]),
            navigation: HashMap::from([
                (C::Up,               vec![Key { key: K::Char('k'), modifiers: M::NONE    }]),
                (C::Down,             vec![Key { key: K::Char('j'), modifiers: M::NONE    }]),
                (C::MoveUp,           vec![Key { key: K::Char('K'), modifiers: M::SHIFT   }]),
                (C::MoveDown,         vec![Key { key: K::Char('J'), modifiers: M::SHIFT   }]),
                (C::Right,            vec![Key { key: K::Char('l'), modifiers: M::NONE    }]),
                (C::Left,             vec![Key { key: K::Char('h'), modifiers: M::NONE    }]),
                (C::DownHalf,         vec![Key { key: K::Char('d'), modifiers: M::CONTROL }]),
                (C::UpHalf,           vec![Key { key: K::Char('u'), modifiers: M::CONTROL }]),
                (C::Bottom,           vec![Key { key: K::Char('G'), modifiers: M::SHIFT   }]),
                (C::Top,              vec![Key { key: K::Char('g'), modifiers: M::NONE    }]),
                (C::EnterSearch,      vec![Key { key: K::Char('/'), modifiers: M::NONE    }]),
                (C::NextResult,       vec![Key { key: K::Char('n'), modifiers: M::NONE    }]),
                (C::PreviousResult,   vec![Key { key: K::Char('N'), modifiers: M::SHIFT   }]),
                (C::Select,           vec![Key { key: K::Char(' '), modifiers: M::NONE    }]),
                (C::Add,              vec![Key { key: K::Char('a'), modifiers: M::NONE    }]),
                (C::Delete,           vec![Key { key: K::Char('D'), modifiers: M::SHIFT   }]),
                (C::Rename,           vec![Key { key: K::Char('r'), modifiers: M::NONE    }]),
                (C::Close,            vec![Key { key: K::Char('c'), modifiers: M::CONTROL }, Key { key: K::Esc, modifiers: M::NONE }]),
                (C::Confirm,          vec![Key { key: K::Enter,     modifiers: M::NONE    }]),
                (C::FocusInput,       vec![Key { key: K::Char('i'), modifiers: M::NONE    }]),
            ]),
            // albums: HashMap::from([
            // ]),
            // artists: HashMap::from([
            // ]),
            // directories: HashMap::from([
            // ]),
            // playlists: HashMap::from([
            // ]),
            #[cfg(debug_assertions)]
            logs: HashMap::from([
                (L::Clear,            vec![Key { key: K::Char('D'), modifiers: M::SHIFT   }]),
            ]),
            queue: HashMap::from([
                (Q::Delete,           vec![Key { key: K::Char('d'), modifiers: M::NONE    }]),
                (Q::DeleteAll,        vec![Key { key: K::Char('D'), modifiers: M::SHIFT   }]),
                (Q::Play,             vec![Key { key: K::Enter,     modifiers: M::NONE    }]),
                (Q::Save,             vec![Key { key: K::Char('s'), modifiers: M::CONTROL }]),
                (Q::AddToPlaylist,    vec![Key { key: K::Char('a'), modifiers: M::NONE    }]),
            ]),
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
            // albums: invert_map(value.albums),
            // artists: invert_map(value.artists),
            // directories: invert_map(value.directories),
            // playlists: invert_map(value.playlists),
            albums: HashMap::new(),
            artists: HashMap::new(),
            directories: HashMap::new(),
            playlists: HashMap::new(),
            search: HashMap::new(),
            #[cfg(debug_assertions)]
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crossterm::event::{KeyCode, KeyModifiers};

    use crate::ui::{
        screens::{logs::LogsActions, queue::QueueActions, CommonAction},
        GlobalAction,
    };

    use super::{Key, KeyConfig, KeyConfigFile};

    #[test]
    #[rustfmt::skip]
    fn converts() {
        let input = KeyConfigFile {
            global: HashMap::from([(GlobalAction::Quit, vec![Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }])]),
            logs: HashMap::from([(LogsActions::Clear, vec![Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }])]),
            queue: HashMap::from([(QueueActions::Play, vec![Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }]),
                                  (QueueActions::Save, vec![Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }])]),
            // albums: HashMap::from([]),
            // artists: HashMap::from([]),
            // directories: HashMap::from([]),
            // playlists: HashMap::from([]),
            navigation: HashMap::from([(CommonAction::Up, vec![Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, 
                                                               Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT }])]),
        };
        let expected = KeyConfig {
            global: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, GlobalAction::Quit)]),
            logs: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, LogsActions::Clear)]),
            queue: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, QueueActions::Play),
                                  (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }, QueueActions::Save)]),
            albums: HashMap::from([]),
            artists: HashMap::from([]),
            directories: HashMap::from([]),
            playlists: HashMap::from([]),
            search: HashMap::from([]),
            navigation: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL }, CommonAction::Up),
                                       (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT }, CommonAction::Up)]),
        };

        let result: KeyConfig = input.into();


        assert_eq!(result, expected);
    }
}
