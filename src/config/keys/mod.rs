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
pub struct KeyConfigFile {
    #[serde(default)]
    pub global: HashMap<Key, GlobalAction>,
    #[serde(default)]
    pub navigation: HashMap<Key, CommonAction>,
    // pub albums: HashMap<AlbumsActions, Vec<Key>>,
    // pub artists: HashMap<ArtistsActions, Vec<Key>>,
    // pub directories: HashMap<DirectoriesActions, Vec<Key>>,
    // pub playlists: HashMap<PlaylistsActions, Vec<Key>>,
    // pub search: HashMap<SearchActions, Vec<Key>>,
    #[cfg(debug_assertions)]
    #[serde(default)]
    pub logs: HashMap<Key, LogsActions>,
    #[serde(default)]
    pub queue: HashMap<Key, QueueActions>,
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
                (Key { key: K::Char('q'), modifiers: M::NONE  }, G::Quit),
                (Key { key: K::Char('>'), modifiers: M::NONE  }, G::NextTrack),
                (Key { key: K::Char('<'), modifiers: M::NONE  }, G::PreviousTrack),
                (Key { key: K::Char('s'), modifiers: M::NONE  }, G::Stop),
                (Key { key: K::Char('z'), modifiers: M::NONE  }, G::ToggleRepeat),
                (Key { key: K::Char('x'), modifiers: M::NONE  }, G::ToggleRandom),
                (Key { key: K::Char('c'), modifiers: M::NONE  }, G::ToggleSingle),
                (Key { key: K::Char('p'), modifiers: M::NONE  }, G::TogglePause),
                (Key { key: K::Char('f'), modifiers: M::NONE  }, G::SeekForward),
                (Key { key: K::Char('b'), modifiers: M::NONE  }, G::SeekBack),
                (Key { key: K::Char(','), modifiers: M::NONE  }, G::VolumeDown),
                (Key { key: K::Char('.'), modifiers: M::NONE  }, G::VolumeUp),
                (Key { key: K::Left,      modifiers: M::NONE  }, G::PreviousTab),
                (Key { key: K::BackTab,   modifiers: M::SHIFT }, G::PreviousTab),
                (Key { key: K::Right,     modifiers: M::NONE  }, G::NextTab),
                (Key { key: K::Tab,       modifiers: M::NONE  }, G::NextTab),
                (Key { key: K::Char('v'), modifiers: M::NONE  }, G::ToggleConsume),
                (Key { key: K::Char('1'), modifiers: M::NONE  }, G::QueueTab),
                (Key { key: K::Char('2'), modifiers: M::NONE  }, G::DirectoriesTab),
                (Key { key: K::Char('3'), modifiers: M::NONE  }, G::ArtistsTab),
                (Key { key: K::Char('4'), modifiers: M::NONE  }, G::AlbumsTab),
                (Key { key: K::Char('5'), modifiers: M::NONE  }, G::PlaylistsTab),
                (Key { key: K::Char('6'), modifiers: M::NONE  }, G::SearchTab),
            ]),
            navigation: HashMap::from([
                (Key { key: K::Char('k'), modifiers: M::NONE    }, C::Up),
                (Key { key: K::Char('j'), modifiers: M::NONE    }, C::Down),
                (Key { key: K::Char('K'), modifiers: M::SHIFT   }, C::MoveUp),
                (Key { key: K::Char('J'), modifiers: M::SHIFT   }, C::MoveDown),
                (Key { key: K::Char('l'), modifiers: M::NONE    }, C::Right),
                (Key { key: K::Char('h'), modifiers: M::NONE    }, C::Left),
                (Key { key: K::Char('d'), modifiers: M::CONTROL }, C::DownHalf),
                (Key { key: K::Char('u'), modifiers: M::CONTROL }, C::UpHalf),
                (Key { key: K::Char('G'), modifiers: M::SHIFT   }, C::Bottom),
                (Key { key: K::Char('g'), modifiers: M::NONE    }, C::Top),
                (Key { key: K::Char('/'), modifiers: M::NONE    }, C::EnterSearch),
                (Key { key: K::Char('n'), modifiers: M::NONE    }, C::NextResult),
                (Key { key: K::Char('N'), modifiers: M::SHIFT   }, C::PreviousResult),
                (Key { key: K::Char(' '), modifiers: M::NONE    }, C::Select),
                (Key { key: K::Char('a'), modifiers: M::NONE    }, C::Add),
                (Key { key: K::Char('D'), modifiers: M::SHIFT   }, C::Delete),
                (Key { key: K::Char('r'), modifiers: M::NONE    }, C::Rename),
                (Key { key: K::Char('c'), modifiers: M::CONTROL }, C::Close),
                (Key { key: K::Esc,       modifiers: M::NONE    }, C::Close),
                (Key { key: K::Enter,     modifiers: M::NONE    }, C::Confirm),
                (Key { key: K::Char('i'), modifiers: M::NONE    }, C::FocusInput),
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
                (Key { key: K::Char('D'), modifiers: M::SHIFT   }, L::Clear),
            ]),
            queue: HashMap::from([
                (Key { key: K::Char('d'), modifiers: M::NONE    }, Q::Delete),
                (Key { key: K::Char('D'), modifiers: M::SHIFT   }, Q::DeleteAll),
                (Key { key: K::Enter,     modifiers: M::NONE    }, Q::Play),
                (Key { key: K::Char('s'), modifiers: M::CONTROL }, Q::Save),
                (Key { key: K::Char('a'), modifiers: M::NONE    }, Q::AddToPlaylist),
            ]),
        }
    }
}

impl From<KeyConfigFile> for KeyConfig {
    fn from(value: KeyConfigFile) -> Self {
        KeyConfig {
            global: value.global,
            navigation: value.navigation,
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
            logs: value.logs,
            queue: value.queue,
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
            global: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, GlobalAction::Quit)]),
            logs: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, LogsActions::Clear)]),
            queue: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, QueueActions::Play),
                                  (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }, QueueActions::Save)]),
            // albums: HashMap::from([]),
            // artists: HashMap::from([]),
            // directories: HashMap::from([]),
            // playlists: HashMap::from([]),
            navigation: HashMap::from([
                (Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, CommonAction::Up),
                (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT }, CommonAction::Up)
            ])
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
