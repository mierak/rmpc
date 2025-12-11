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
    QueueActions,
    SearchActions,
};
use actions::{
    AlbumsActionsFile,
    ArtistsActionsFile,
    CommonActionFile,
    DirectoriesActionsFile,
    GlobalActionFile,
    QueueActionsFile,
};
use crossterm::event::{KeyCode, KeyModifiers};
pub use key::Key;
use serde::{Deserialize, Serialize};

use super::defaults;
use crate::config::keys::actions::SaveKind;

pub(crate) mod actions;
pub mod key;

#[derive(Debug, PartialEq, Clone)]
pub struct KeyConfig {
    pub global: HashMap<Key, GlobalAction>,
    pub navigation: HashMap<Key, CommonAction>,
    pub albums: HashMap<Key, AlbumsActions>,
    pub artists: HashMap<Key, ArtistsActions>,
    pub directories: HashMap<Key, DirectoriesActions>,
    pub search: HashMap<Key, SearchActions>,
    #[cfg(debug_assertions)]
    pub logs: HashMap<Key, LogsActions>,
    pub queue: HashMap<Key, QueueActions>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyConfigFile {
    #[serde(default = "defaults::bool::<false>")]
    pub clear: bool,
    #[serde(default)]
    pub global: HashMap<Key, GlobalActionFile>,
    #[serde(default)]
    pub navigation: HashMap<Key, CommonActionFile>,
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
        use KeyCode as K;
        use KeyModifiers as M;
        #[cfg(debug_assertions)]
        use LogsActionsFile as L;
        use QueueActionsFile as Q;
        Self {
            clear: false,
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
                (Key { key: K::Char('R'), modifiers: M::SHIFT }, G::AddRandom),
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
                (Key { key: K::Char('B'), modifiers: M::SHIFT   }, C::ShowInfo),
                (Key { key: K::Char('z'), modifiers: M::CONTROL }, C::ContextMenu {}),
                (Key { key: K::Char('s'), modifiers: M::CONTROL }, C::Save { kind: SaveKind::default() }),
            ]),
            #[cfg(debug_assertions)]
            logs: HashMap::from([
                (Key { key: K::Char('D'), modifiers: M::SHIFT   }, L::Clear),
                (Key { key: K::Char('S'), modifiers: M::SHIFT   }, L::ToggleScroll),
            ]),
            queue: HashMap::from([
                (Key { key: K::Char('d'), modifiers: M::NONE    }, Q::Delete),
                (Key { key: K::Char('D'), modifiers: M::SHIFT   }, Q::DeleteAll),
                (Key { key: K::Enter,     modifiers: M::NONE    }, Q::Play),
                (Key { key: K::Char('a'), modifiers: M::NONE    }, Q::AddToPlaylist),
                (Key { key: K::Char('C'), modifiers: M::SHIFT   }, Q::JumpToCurrent),
                (Key { key: K::Char('X'), modifiers: M::SHIFT   }, Q::Shuffle),
            ]),
        }
    }
}

impl Default for KeyConfig {
    fn default() -> Self {
        KeyConfigFile { clear: true, ..Default::default() }
            .try_into()
            .expect("Default KeyConfigFile should convert to KeyConfig")
    }
}

impl TryFrom<KeyConfigFile> for KeyConfig {
    type Error = anyhow::Error;

    fn try_from(value: KeyConfigFile) -> Result<Self, Self::Error> {
        if value.clear {
            Ok(KeyConfig {
                global: value.global.into_iter().map(|(k, v)| (k, v.into())).collect(),
                navigation: value
                    .navigation
                    .into_iter()
                    .map(|(k, v)| -> anyhow::Result<_> { Ok((k, v.try_into()?)) })
                    .collect::<anyhow::Result<_>>()?,
                albums: HashMap::new(),
                artists: HashMap::new(),
                directories: HashMap::new(),
                search: HashMap::new(),
                #[cfg(debug_assertions)]
                logs: value.logs.into_iter().map(|(k, v)| (k, v.into())).collect(),
                queue: value.queue.into_iter().map(|(k, v)| (k, v.into())).collect(),
            })
        } else {
            let global: HashMap<Key, GlobalAction> =
                value.global.into_iter().map(|(k, v)| (k, v.into())).collect();
            let navigation: HashMap<Key, CommonAction> = value
                .navigation
                .into_iter()
                .map(|(k, v)| -> anyhow::Result<_> { Ok((k, v.try_into()?)) })
                .collect::<anyhow::Result<_>>()?;
            let queue: HashMap<Key, QueueActions> =
                value.queue.into_iter().map(|(k, v)| (k, v.into())).collect();
            #[cfg(debug_assertions)]
            let logs: HashMap<Key, LogsActions> =
                value.logs.into_iter().map(|(k, v)| (k, v.into())).collect();

            let mut result = KeyConfig::default();

            for (k, v) in global {
                result.global.insert(k, v);
            }

            for (k, v) in navigation {
                result.navigation.insert(k, v);
            }

            for (k, v) in queue {
                result.queue.insert(k, v);
            }

            #[cfg(debug_assertions)]
            for (k, v) in logs {
                result.logs.insert(k, v);
            }

            Ok(result)
        }
    }
}

pub trait ToDescription {
    fn to_description(&self) -> Cow<'static, str>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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
            clear: true,
            global: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, GlobalActionFile::Quit)]),

            #[cfg(debug_assertions)]
            logs: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, LogsActionsFile::Clear)]),
            queue: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, QueueActionsFile::Play),
                                  (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }, QueueActionsFile::Save)]),
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
            search: HashMap::from([]),
            navigation: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL }, CommonAction::Up),
                                       (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT }, CommonAction::Up)]),
        };

        let result: KeyConfig = input.try_into().unwrap();


        assert_eq!(result, expected);
    }

    #[test]
    #[rustfmt::skip]
    fn converts_without_clearing() {
        let input = KeyConfigFile {
            clear: false,
            global: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, GlobalActionFile::Quit)]),
            queue: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, QueueActionsFile::Play),
                                  (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }, QueueActionsFile::Save)]),
            navigation: HashMap::from([
                (Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, CommonActionFile::Up),
                (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT }, CommonActionFile::Up)
            ]),
            #[cfg(debug_assertions)]
            logs: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, LogsActionsFile::Clear)]),
        };

        let mut default: KeyConfig = KeyConfig::default();
        default.global.insert(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, GlobalAction::Quit);
        default.queue.insert(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, QueueActions::Play);
        default.queue.insert(Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }, QueueActions::Save);
        default.navigation.insert(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL }, CommonAction::Up);
        default.navigation.insert(Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT }, CommonAction::Up);
        #[cfg(debug_assertions)]
        default.logs.insert(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }, LogsActions::Clear);

        let result: KeyConfig = input.try_into().unwrap();

        assert_eq!(result, default);
    }
}
