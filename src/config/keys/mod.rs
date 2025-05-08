use std::{borrow::Cow, collections::HashMap};

#[cfg(debug_assertions)]
pub use actions::LogsActions;
#[cfg(debug_assertions)]
use actions::LogsActionsFile;
pub use actions::{
    AlbumsActions,
    ArtistsActions,
    CommonAction,
    DirectoriesActions,
    GlobalAction,
    PlaylistsActions,
    QueueActions,
    SearchActions,
};
use actions::{
    AlbumsActionsFile,
    ArtistsActionsFile,
    CommonActionFile,
    DirectoriesActionsFile,
    GlobalActionFile,
    PlaylistsActionsFile,
    QueueActionsFile,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
pub use key::Key;
use serde::{Deserialize, Serialize};

mod actions;
mod key;

#[derive(Debug, PartialEq, Default, Clone)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyConfigFile {
    #[serde(default)]
    pub global: HashMap<Key, GlobalActionFile>,
    #[serde(default)]
    pub navigation: HashMap<Key, CommonActionFile>,
    // pub albums: HashMap<AlbumsActions, Vec<Key>>,
    // pub artists: HashMap<ArtistsActions, Vec<Key>>,
    // pub directories: HashMap<DirectoriesActions, Vec<Key>>,
    // pub playlists: HashMap<PlaylistsActions, Vec<Key>>,
    // pub search: HashMap<SearchActions, Vec<Key>>,
    #[cfg(debug_assertions)]
    #[serde(default)]
    pub logs: HashMap<Key, LogsActionsFile>,
    #[serde(default)]
    pub queue: HashMap<Key, QueueActionsFile>,
}

impl Default for KeyConfigFile {
    #[rustfmt::skip]
    #[allow(unused_imports)]
    fn default() -> Self {
        use GlobalActionFile as G;
        use CommonActionFile as C;
        use AlbumsActionsFile as Al;
        use ArtistsActionsFile as Ar;
        use DirectoriesActionsFile  as D;
        use PlaylistsActionsFile as P;
        use KeyCode as K;
        use KeyModifiers as M;
        #[cfg(debug_assertions)]
        use LogsActionsFile as L;
        use QueueActionsFile as Q;
        Self {
            global: HashMap::from([
                (Key { key: K::Char('q'), modifiers: M::NONE  }, G::Quit),
                (Key { key: K::Char(':'), modifiers: M::NONE  }, G::CommandMode),
                (Key { key: K::Char('~'), modifiers: M::NONE  }, G::ShowHelp),
                (Key { key: K::Char('I'), modifiers: M::SHIFT }, G::ShowCurrentSongInfo),
                (Key { key: K::Char('O'), modifiers: M::SHIFT }, G::ShowOutputs),
                (Key { key: K::Char('P'), modifiers: M::SHIFT }, G::ShowDecoders),
                (Key { key: K::Char('>'), modifiers: M::NONE  }, G::NextTrack),
                (Key { key: K::Char('<'), modifiers: M::NONE  }, G::PreviousTrack),
                (Key { key: K::Char('s'), modifiers: M::NONE  }, G::Stop),
                (Key { key: K::Char('z'), modifiers: M::NONE  }, G::ToggleRepeat),
                (Key { key: K::Char('x'), modifiers: M::NONE  }, G::ToggleRandom),
                (Key { key: K::Char('c'), modifiers: M::NONE  }, G::ToggleConsume),
                (Key { key: K::Char('v'), modifiers: M::NONE  }, G::ToggleSingle),
                (Key { key: K::Char('p'), modifiers: M::NONE  }, G::TogglePause),
                (Key { key: K::Char('f'), modifiers: M::NONE  }, G::SeekForward),
                (Key { key: K::Char('b'), modifiers: M::NONE  }, G::SeekBack),
                (Key { key: K::Char('u'), modifiers: M::NONE  }, G::Update),
                (Key { key: K::Char('U'), modifiers: M::SHIFT }, G::Rescan),
                (Key { key: K::Char(','), modifiers: M::NONE  }, G::VolumeDown),
                (Key { key: K::Char('.'), modifiers: M::NONE  }, G::VolumeUp),
                (Key { key: K::BackTab,   modifiers: M::SHIFT }, G::PreviousTab),
                (Key { key: K::Tab,       modifiers: M::NONE  }, G::NextTab),
                (Key { key: K::Char('1'), modifiers: M::NONE  }, G::SwitchToTab("Queue".to_string())),
                (Key { key: K::Char('2'), modifiers: M::NONE  }, G::SwitchToTab("Directories".to_string())),
                (Key { key: K::Char('3'), modifiers: M::NONE  }, G::SwitchToTab("Artists".to_string())),
                (Key { key: K::Char('4'), modifiers: M::NONE  }, G::SwitchToTab("Album Artists".to_string())),
                (Key { key: K::Char('5'), modifiers: M::NONE  }, G::SwitchToTab("Albums".to_string())),
                (Key { key: K::Char('6'), modifiers: M::NONE  }, G::SwitchToTab("Playlists".to_string())),
                (Key { key: K::Char('7'), modifiers: M::NONE  }, G::SwitchToTab("Search".to_string())),
            ]),
            navigation: HashMap::from([
                (Key { key: K::Char('k'), modifiers: M::NONE    }, C::Up),
                (Key { key: K::Char('j'), modifiers: M::NONE    }, C::Down),
                (Key { key: K::Char('l'), modifiers: M::NONE    }, C::Right),
                (Key { key: K::Left,      modifiers: M::NONE    }, C::Left),
                (Key { key: K::Up,        modifiers: M::NONE    }, C::Up),
                (Key { key: K::Down,      modifiers: M::NONE    }, C::Down),
                (Key { key: K::Right,     modifiers: M::NONE    }, C::Right),
                (Key { key: K::Char('h'), modifiers: M::NONE    }, C::Left),
                (Key { key: K::Char('k'), modifiers: M::CONTROL }, C::PaneUp),
                (Key { key: K::Char('j'), modifiers: M::CONTROL }, C::PaneDown),
                (Key { key: K::Char('l'), modifiers: M::CONTROL }, C::PaneRight),
                (Key { key: K::Char('h'), modifiers: M::CONTROL }, C::PaneLeft),
                (Key { key: K::Char('K'), modifiers: M::SHIFT   }, C::MoveUp),
                (Key { key: K::Char('J'), modifiers: M::SHIFT   }, C::MoveDown),
                (Key { key: K::Char('d'), modifiers: M::CONTROL }, C::DownHalf),
                (Key { key: K::Char('u'), modifiers: M::CONTROL }, C::UpHalf),
                (Key { key: K::Char('G'), modifiers: M::SHIFT   }, C::Bottom),
                (Key { key: K::Char('g'), modifiers: M::NONE    }, C::Top),
                (Key { key: K::Char('/'), modifiers: M::NONE    }, C::EnterSearch),
                (Key { key: K::Char('n'), modifiers: M::NONE    }, C::NextResult),
                (Key { key: K::Char('N'), modifiers: M::SHIFT   }, C::PreviousResult),
                (Key { key: K::Char(' '), modifiers: M::NONE    }, C::Select),
                (Key { key: K::Char(' '), modifiers: M::CONTROL }, C::InvertSelection),
                (Key { key: K::Char('a'), modifiers: M::NONE    }, C::Add),
                (Key { key: K::Char('A'), modifiers: M::SHIFT   }, C::AddAll),
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
                (Key { key: K::Char('S'), modifiers: M::SHIFT   }, L::ToggleScroll),
            ]),
            queue: HashMap::from([
                (Key { key: K::Char('d'), modifiers: M::NONE    }, Q::Delete),
                (Key { key: K::Char('D'), modifiers: M::SHIFT   }, Q::DeleteAll),
                (Key { key: K::Enter,     modifiers: M::NONE    }, Q::Play),
                (Key { key: K::Char('s'), modifiers: M::CONTROL }, Q::Save),
                (Key { key: K::Char('a'), modifiers: M::NONE    }, Q::AddToPlaylist),
                (Key { key: K::Char('i'), modifiers: M::NONE    }, Q::ShowInfo),
                (Key { key: K::Char('C'), modifiers: M::SHIFT   }, Q::JumpToCurrent),
            ]),
        }
    }
}

impl From<KeyConfigFile> for KeyConfig {
    fn from(value: KeyConfigFile) -> Self {
        KeyConfig {
            global: value.global.into_iter().map(|(k, v)| (k, v.into())).collect(),
            navigation: value.navigation.into_iter().map(|(k, v)| (k, v.into())).collect(),
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
            logs: value.logs.into_iter().map(|(k, v)| (k, v.into())).collect(),
            queue: value.queue.into_iter().map(|(k, v)| (k, v.into())).collect(),
        }
    }
}

impl From<KeyEvent> for Key {
    fn from(value: KeyEvent) -> Self {
        Self { key: value.code, modifiers: value.modifiers }
    }
}

pub trait ToDescription {
    fn to_description(&self) -> Cow<'static, str>;
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crossterm::event::{KeyCode, KeyModifiers};

    use super::{Key, KeyConfig, KeyConfigFile};
    #[cfg(debug_assertions)]
    use crate::config::keys::LogsActions;
    #[cfg(debug_assertions)]
    use crate::config::keys::LogsActionsFile;
    use crate::config::keys::{
        CommonAction,
        GlobalAction,
        QueueActions,
        actions::{CommonActionFile, GlobalActionFile, QueueActionsFile},
    };

    #[test]
    #[rustfmt::skip]
    fn converts() {
        let input = KeyConfigFile {
            global: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, GlobalActionFile::Quit)]),

            #[cfg(debug_assertions)]
            logs: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, LogsActionsFile::Clear)]),
            queue: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, QueueActionsFile::Play),
                                  (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }, QueueActionsFile::Save)]),
            // albums: HashMap::from([]),
            // artists: HashMap::from([]),
            // directories: HashMap::from([]),
            // playlists: HashMap::from([]),
            navigation: HashMap::from([
                (Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, CommonActionFile::Up),
                (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT }, CommonActionFile::Up)
            ])
        };
        let expected = KeyConfig {
            global: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, GlobalAction::Quit)]),
            #[cfg(debug_assertions)]
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
