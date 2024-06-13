use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use itertools::Itertools;
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

use self::key::Key;

mod key;

#[derive(Debug, PartialEq, Default)]
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum SingleOrMultiple<T> {
    Single(T),
    Multiple(Vec<T>),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyConfigFile {
    #[serde(default)]
    pub global: HashMap<GlobalAction, SingleOrMultiple<Key>>,
    #[serde(default)]
    pub navigation: HashMap<CommonAction, SingleOrMultiple<Key>>,
    // pub albums: HashMap<AlbumsActions, Vec<Key>>,
    // pub artists: HashMap<ArtistsActions, Vec<Key>>,
    // pub directories: HashMap<DirectoriesActions, Vec<Key>>,
    // pub playlists: HashMap<PlaylistsActions, Vec<Key>>,
    // pub search: HashMap<SearchActions, Vec<Key>>,
    #[cfg(debug_assertions)]
    #[serde(default)]
    pub logs: HashMap<LogsActions, SingleOrMultiple<Key>>,
    #[serde(default)]
    pub queue: HashMap<QueueActions, SingleOrMultiple<Key>>,
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
                (G::Quit,             SingleOrMultiple::Single(Key { key: K::Char('q'), modifiers: M::NONE })),
                (G::NextTrack,        SingleOrMultiple::Single(Key { key: K::Char('>'), modifiers: M::NONE })),
                (G::PreviousTrack,    SingleOrMultiple::Single(Key { key: K::Char('<'), modifiers: M::NONE })),
                (G::Stop,             SingleOrMultiple::Single(Key { key: K::Char('s'), modifiers: M::NONE })),
                (G::ToggleRepeat,     SingleOrMultiple::Single(Key { key: K::Char('z'), modifiers: M::NONE })),
                (G::ToggleRandom,     SingleOrMultiple::Single(Key { key: K::Char('x'), modifiers: M::NONE })),
                (G::ToggleSingle,     SingleOrMultiple::Single(Key { key: K::Char('c'), modifiers: M::NONE })),
                (G::TogglePause,      SingleOrMultiple::Single(Key { key: K::Char('p'), modifiers: M::NONE })),
                (G::SeekForward,      SingleOrMultiple::Single(Key { key: K::Char('f'), modifiers: M::NONE })),
                (G::SeekBack,         SingleOrMultiple::Single(Key { key: K::Char('b'), modifiers: M::NONE })),
                (G::VolumeDown,       SingleOrMultiple::Single(Key { key: K::Char(','), modifiers: M::NONE })),
                (G::VolumeUp,         SingleOrMultiple::Single(Key { key: K::Char('.'), modifiers: M::NONE })),
                (G::PreviousTab,      SingleOrMultiple::Multiple(vec![Key { key: K::Left,      modifiers: M::NONE }, Key { key: K::BackTab,  modifiers: M::SHIFT }])),
                (G::NextTab,          SingleOrMultiple::Multiple(vec![Key { key: K::Right,     modifiers: M::NONE }, Key { key: K::Tab,      modifiers: M::NONE }])),
                (G::ToggleConsume,    SingleOrMultiple::Single(Key { key: K::Char('v'), modifiers: M::NONE })),
                (G::QueueTab,         SingleOrMultiple::Single(Key { key: K::Char('1'), modifiers: M::NONE })),
                (G::DirectoriesTab,   SingleOrMultiple::Single(Key { key: K::Char('2'), modifiers: M::NONE })),
                (G::ArtistsTab,       SingleOrMultiple::Single(Key { key: K::Char('3'), modifiers: M::NONE })),
                (G::AlbumsTab,        SingleOrMultiple::Single(Key { key: K::Char('4'), modifiers: M::NONE })),
                (G::PlaylistsTab,     SingleOrMultiple::Single(Key { key: K::Char('5'), modifiers: M::NONE })),
                (G::SearchTab,        SingleOrMultiple::Single(Key { key: K::Char('6'), modifiers: M::NONE })),
            ]),
            navigation: HashMap::from([
                (C::Up,               SingleOrMultiple::Single(Key { key: K::Char('k'), modifiers: M::NONE    })),
                (C::Down,             SingleOrMultiple::Single(Key { key: K::Char('j'), modifiers: M::NONE    })),
                (C::MoveUp,           SingleOrMultiple::Single(Key { key: K::Char('K'), modifiers: M::SHIFT   })),
                (C::MoveDown,         SingleOrMultiple::Single(Key { key: K::Char('J'), modifiers: M::SHIFT   })),
                (C::Right,            SingleOrMultiple::Single(Key { key: K::Char('l'), modifiers: M::NONE    })),
                (C::Left,             SingleOrMultiple::Single(Key { key: K::Char('h'), modifiers: M::NONE    })),
                (C::DownHalf,         SingleOrMultiple::Single(Key { key: K::Char('d'), modifiers: M::CONTROL })),
                (C::UpHalf,           SingleOrMultiple::Single(Key { key: K::Char('u'), modifiers: M::CONTROL })),
                (C::Bottom,           SingleOrMultiple::Single(Key { key: K::Char('G'), modifiers: M::SHIFT   })),
                (C::Top,              SingleOrMultiple::Single(Key { key: K::Char('g'), modifiers: M::NONE    })),
                (C::EnterSearch,      SingleOrMultiple::Single(Key { key: K::Char('/'), modifiers: M::NONE    })),
                (C::NextResult,       SingleOrMultiple::Single(Key { key: K::Char('n'), modifiers: M::NONE    })),
                (C::PreviousResult,   SingleOrMultiple::Single(Key { key: K::Char('N'), modifiers: M::SHIFT   })),
                (C::Select,           SingleOrMultiple::Single(Key { key: K::Char(' '), modifiers: M::NONE    })),
                (C::Add,              SingleOrMultiple::Single(Key { key: K::Char('a'), modifiers: M::NONE    })),
                (C::Delete,           SingleOrMultiple::Single(Key { key: K::Char('D'), modifiers: M::SHIFT   })),
                (C::Rename,           SingleOrMultiple::Single(Key { key: K::Char('r'), modifiers: M::NONE    })),
                (C::Close,            SingleOrMultiple::Multiple(vec![Key { key: K::Char('c'), modifiers: M::CONTROL }, Key { key: K::Esc, modifiers: M::NONE }])),
                (C::Confirm,          SingleOrMultiple::Single(Key { key: K::Enter,     modifiers: M::NONE    })),
                (C::FocusInput,       SingleOrMultiple::Single(Key { key: K::Char('i'), modifiers: M::NONE    })),
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
                (L::Clear,            SingleOrMultiple::Single(Key { key: K::Char('D'), modifiers: M::SHIFT   })),
            ]),
            queue: HashMap::from([
                (Q::Delete,           SingleOrMultiple::Single(Key { key: K::Char('d'), modifiers: M::NONE    })),
                (Q::DeleteAll,        SingleOrMultiple::Single(Key { key: K::Char('D'), modifiers: M::SHIFT   })),
                (Q::Play,             SingleOrMultiple::Single(Key { key: K::Enter,     modifiers: M::NONE    })),
                (Q::Save,             SingleOrMultiple::Single(Key { key: K::Char('s'), modifiers: M::CONTROL })),
                (Q::AddToPlaylist,    SingleOrMultiple::Single(Key { key: K::Char('a'), modifiers: M::NONE    })),
            ]),
        }
    }
}

fn invert_and_flatten<T: Copy, V: std::hash::Hash + std::cmp::Eq>(
    v: HashMap<T, SingleOrMultiple<V>>,
) -> impl Iterator<Item = (V, T)> {
    v.into_iter().flat_map(|(k, v)| match v {
        SingleOrMultiple::Single(v) => vec![(v, k)],
        SingleOrMultiple::Multiple(v) => v.into_iter().map(move |v| (v, k)).collect_vec(),
    })
}

fn invert_keys<T: Copy>(v: HashMap<T, SingleOrMultiple<Key>>) -> HashMap<Key, T> {
    invert_and_flatten(v).filter(|v| v.0.key != KeyCode::Null).collect()
}

impl From<KeyConfigFile> for KeyConfig {
    fn from(value: KeyConfigFile) -> Self {
        KeyConfig {
            global: invert_keys(value.global),
            navigation: invert_keys(value.navigation),
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
            logs: invert_keys(value.logs),
            queue: invert_keys(value.queue),
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

    use crate::{
        config::keys::SingleOrMultiple,
        ui::{
            screens::{logs::LogsActions, queue::QueueActions, CommonAction},
            GlobalAction,
        },
    };

    use super::{Key, KeyConfig, KeyConfigFile};

    #[test]
    #[rustfmt::skip]
    fn converts() {
        let input = KeyConfigFile {
            global: HashMap::from([(GlobalAction::Quit, SingleOrMultiple::Single(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }))]),
            logs: HashMap::from([(LogsActions::Clear, SingleOrMultiple::Single(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }))]),
            queue: HashMap::from([(QueueActions::Play, SingleOrMultiple::Single(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, })),
                                  (QueueActions::Save, SingleOrMultiple::Single(Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }))]),
            // albums: HashMap::from([]),
            // artists: HashMap::from([]),
            // directories: HashMap::from([]),
            // playlists: HashMap::from([]),
            navigation: HashMap::from([(CommonAction::Up, SingleOrMultiple::Multiple(vec![Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, },
                                                               Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT }]))]),
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
