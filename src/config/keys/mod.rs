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
use crate::config::keys::{actions::SaveKind, key::KeySequence};

pub(crate) mod actions;
pub mod key;

#[derive(Debug, PartialEq, Clone)]
pub struct KeyConfig {
    pub global: HashMap<KeySequence, GlobalAction>,
    pub navigation: HashMap<KeySequence, CommonAction>,
    pub albums: HashMap<KeySequence, AlbumsActions>,
    pub artists: HashMap<KeySequence, ArtistsActions>,
    pub directories: HashMap<KeySequence, DirectoriesActions>,
    pub search: HashMap<KeySequence, SearchActions>,
    #[cfg(debug_assertions)]
    pub logs: HashMap<KeySequence, LogsActions>,
    pub queue: HashMap<KeySequence, QueueActions>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyConfigFile {
    #[serde(default = "defaults::bool::<false>")]
    pub clear: bool,
    #[serde(default)]
    pub global: HashMap<KeySequence, GlobalActionFile>,
    #[serde(default)]
    pub navigation: HashMap<KeySequence, CommonActionFile>,
    #[cfg(debug_assertions)]
    #[serde(default)]
    pub logs: HashMap<KeySequence, LogsActionsFile>,
    #[serde(default)]
    pub queue: HashMap<KeySequence, QueueActionsFile>,
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
                (Key { key: K::Char('q'), modifiers: M::NONE  }.into(), G::Quit),
                (Key { key: K::Char(':'), modifiers: M::NONE  }.into(), G::CommandMode),
                (Key { key: K::Char('~'), modifiers: M::NONE  }.into(), G::ShowHelp),
                (Key { key: K::Char('I'), modifiers: M::SHIFT }.into(), G::ShowCurrentSongInfo),
                (Key { key: K::Char('O'), modifiers: M::SHIFT }.into(), G::ShowOutputs),
                (Key { key: K::Char('P'), modifiers: M::SHIFT }.into(), G::ShowDecoders),
                (Key { key: K::Char('>'), modifiers: M::NONE  }.into(), G::NextTrack),
                (Key { key: K::Char('<'), modifiers: M::NONE  }.into(), G::PreviousTrack),
                (Key { key: K::Char('s'), modifiers: M::NONE  }.into(), G::Stop),
                (Key { key: K::Char('z'), modifiers: M::NONE  }.into(), G::ToggleRepeat),
                (Key { key: K::Char('x'), modifiers: M::NONE  }.into(), G::ToggleRandom),
                (Key { key: K::Char('c'), modifiers: M::NONE  }.into(), G::ToggleConsume),
                (Key { key: K::Char('v'), modifiers: M::NONE  }.into(), G::ToggleSingle),
                (Key { key: K::Char('p'), modifiers: M::NONE  }.into(), G::TogglePause),
                (Key { key: K::Char('f'), modifiers: M::NONE  }.into(), G::SeekForward),
                (Key { key: K::Char('b'), modifiers: M::NONE  }.into(), G::SeekBack),
                (Key { key: K::Char('u'), modifiers: M::NONE  }.into(), G::Update),
                (Key { key: K::Char('U'), modifiers: M::SHIFT }.into(), G::Rescan),
                (Key { key: K::Char(','), modifiers: M::NONE  }.into(), G::VolumeDown),
                (Key { key: K::Char('.'), modifiers: M::NONE  }.into(), G::VolumeUp),
                (Key { key: K::BackTab,   modifiers: M::SHIFT }.into(), G::PreviousTab),
                (Key { key: K::Tab,       modifiers: M::NONE  }.into(), G::NextTab),
                (Key { key: K::Char('R'), modifiers: M::SHIFT }.into(), G::AddRandom),
                (Key { key: K::Char('1'), modifiers: M::NONE  }.into(), G::SwitchToTab("Queue".to_string())),
                (Key { key: K::Char('2'), modifiers: M::NONE  }.into(), G::SwitchToTab("Directories".to_string())),
                (Key { key: K::Char('3'), modifiers: M::NONE  }.into(), G::SwitchToTab("Artists".to_string())),
                (Key { key: K::Char('4'), modifiers: M::NONE  }.into(), G::SwitchToTab("Album Artists".to_string())),
                (Key { key: K::Char('5'), modifiers: M::NONE  }.into(), G::SwitchToTab("Albums".to_string())),
                (Key { key: K::Char('6'), modifiers: M::NONE  }.into(), G::SwitchToTab("Playlists".to_string())),
                (Key { key: K::Char('7'), modifiers: M::NONE  }.into(), G::SwitchToTab("Search".to_string())),
            ]),
            navigation: HashMap::from([
                (Key { key: K::Char('k'), modifiers: M::NONE    }.into(), C::Up),
                (Key { key: K::Char('j'), modifiers: M::NONE    }.into(), C::Down),
                (Key { key: K::Char('l'), modifiers: M::NONE    }.into(), C::Right),
                (Key { key: K::Left,      modifiers: M::NONE    }.into(), C::Left),
                (Key { key: K::Up,        modifiers: M::NONE    }.into(), C::Up),
                (Key { key: K::Down,      modifiers: M::NONE    }.into(), C::Down),
                (Key { key: K::Right,     modifiers: M::NONE    }.into(), C::Right),
                (Key { key: K::Char('h'), modifiers: M::NONE    }.into(), C::Left),
                (Key { key: K::Char('k'), modifiers: M::CONTROL }.into(), C::PaneUp),
                (Key { key: K::Char('j'), modifiers: M::CONTROL }.into(), C::PaneDown),
                (Key { key: K::Char('l'), modifiers: M::CONTROL }.into(), C::PaneRight),
                (Key { key: K::Char('h'), modifiers: M::CONTROL }.into(), C::PaneLeft),
                (Key { key: K::Char('K'), modifiers: M::SHIFT   }.into(), C::MoveUp),
                (Key { key: K::Char('J'), modifiers: M::SHIFT   }.into(), C::MoveDown),
                (Key { key: K::Char('d'), modifiers: M::CONTROL }.into(), C::DownHalf),
                (Key { key: K::Char('u'), modifiers: M::CONTROL }.into(), C::UpHalf),
                (Key { key: K::Char('G'), modifiers: M::SHIFT   }.into(), C::Bottom),
                (Key { key: K::Char('g'), modifiers: M::NONE    }.into(), C::Top),
                (Key { key: K::Char('/'), modifiers: M::NONE    }.into(), C::EnterSearch),
                (Key { key: K::Char('n'), modifiers: M::NONE    }.into(), C::NextResult),
                (Key { key: K::Char('N'), modifiers: M::SHIFT   }.into(), C::PreviousResult),
                (Key { key: K::Char(' '), modifiers: M::NONE    }.into(), C::Select),
                (Key { key: K::Char(' '), modifiers: M::CONTROL }.into(), C::InvertSelection),
                (Key { key: K::Char('a'), modifiers: M::NONE    }.into(), C::Add),
                (Key { key: K::Char('A'), modifiers: M::SHIFT   }.into(), C::AddAll),
                (Key { key: K::Char('D'), modifiers: M::SHIFT   }.into(), C::Delete),
                (Key { key: K::Char('r'), modifiers: M::NONE    }.into(), C::Rename),
                (Key { key: K::Char('c'), modifiers: M::CONTROL }.into(), C::Close),
                (Key { key: K::Esc,       modifiers: M::NONE    }.into(), C::Close),
                (Key { key: K::Enter,     modifiers: M::NONE    }.into(), C::Confirm),
                (Key { key: K::Char('i'), modifiers: M::NONE    }.into(), C::FocusInput),
                (Key { key: K::Char('B'), modifiers: M::SHIFT   }.into(), C::ShowInfo),
                (Key { key: K::Char('z'), modifiers: M::CONTROL }.into(), C::ContextMenu {}),
                (Key { key: K::Char('s'), modifiers: M::CONTROL }.into(), C::Save { kind: SaveKind::default() }),
            ]),
            #[cfg(debug_assertions)]
            logs: HashMap::from([
                (Key { key: K::Char('D'), modifiers: M::SHIFT   }.into(), L::Clear),
                (Key { key: K::Char('S'), modifiers: M::SHIFT   }.into(), L::ToggleScroll),
            ]),
            queue: HashMap::from([
                (Key { key: K::Char('d'), modifiers: M::NONE    }.into(), Q::Delete),
                (Key { key: K::Char('D'), modifiers: M::SHIFT   }.into(), Q::DeleteAll),
                (Key { key: K::Enter,     modifiers: M::NONE    }.into(), Q::Play),
                (Key { key: K::Char('a'), modifiers: M::NONE    }.into(), Q::AddToPlaylist),
                (Key { key: K::Char('C'), modifiers: M::SHIFT   }.into(), Q::JumpToCurrent),
                (Key { key: K::Char('X'), modifiers: M::SHIFT   }.into(), Q::Shuffle),
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
            let global: HashMap<KeySequence, GlobalAction> =
                value.global.into_iter().map(|(k, v)| (k, v.into())).collect();
            let navigation: HashMap<KeySequence, CommonAction> = value
                .navigation
                .into_iter()
                .map(|(k, v)| -> anyhow::Result<_> { Ok((k, v.try_into()?)) })
                .collect::<anyhow::Result<_>>()?;
            let queue: HashMap<KeySequence, QueueActions> =
                value.queue.into_iter().map(|(k, v)| (k, v.into())).collect();
            #[cfg(debug_assertions)]
            let logs: HashMap<KeySequence, LogsActions> =
                value.logs.into_iter().map(|(k, v)| (k, v.into())).collect();

            let mut result = KeyConfig::default();

            let all_key_overrides = global.keys().chain(navigation.keys()).chain(queue.keys());
            #[cfg(debug_assertions)]
            let all_key_overrides = all_key_overrides.chain(logs.keys());
            for key in all_key_overrides {
                result.global.remove(key);
                result.navigation.remove(key);
                result.queue.remove(key);
                #[cfg(debug_assertions)]
                result.logs.remove(key);
            }

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
            global: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), GlobalActionFile::Quit)]),

            #[cfg(debug_assertions)]
            logs: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), LogsActionsFile::Clear)]),
            queue: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), QueueActionsFile::Play),
                                  (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }.into(), QueueActionsFile::Save)]),
            navigation: HashMap::from([
                (Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), CommonActionFile::Up),
                (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT }.into(), CommonActionFile::Up)
            ])
        };
        let expected = KeyConfig {
            global: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), GlobalAction::Quit)]),
            #[cfg(debug_assertions)]
            logs: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), LogsActions::Clear)]),
            queue: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), QueueActions::Play),
                                  (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }.into(), QueueActions::Save)]),
            albums: HashMap::from([]),
            artists: HashMap::from([]),
            directories: HashMap::from([]),
            search: HashMap::from([]),
            navigation: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL }.into(), CommonAction::Up),
                                       (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT }.into(), CommonAction::Up)]),
        };

        let result: KeyConfig = input.try_into().unwrap();


        assert_eq!(result, expected);
    }

    #[test]
    #[rustfmt::skip]
    fn converts_without_clearing() {
        let input = KeyConfigFile {
            clear: false,
            global: HashMap::from([
                (Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), GlobalActionFile::Quit),
                (Key { key: KeyCode::Char(' '), modifiers: KeyModifiers::NONE }.into(), GlobalActionFile::TogglePause),
            ]),
            queue: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), QueueActionsFile::Play),
                                  (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }.into(), QueueActionsFile::Save)]),
            navigation: HashMap::from([
                (Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), CommonActionFile::Up),
                (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT }.into(), CommonActionFile::Up),
            ]),
            #[cfg(debug_assertions)]
            logs: HashMap::from([(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), LogsActionsFile::Clear)]),
        };

        let mut default: KeyConfig = KeyConfig::default();
        default.global.insert(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), GlobalAction::Quit);
        default.queue.insert(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), QueueActions::Play);
        default.queue.insert(Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }.into(), QueueActions::Save);
        default.navigation.insert(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL }.into(), CommonAction::Up);
        default.navigation.insert(Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT }.into(), CommonAction::Up);
        #[cfg(debug_assertions)]
        default.logs.insert(Key { key: KeyCode::Char('a'), modifiers: KeyModifiers::CONTROL, }.into(), LogsActions::Clear);

        // <Space> is mapped in global keys, it has to remove the default `Select` mapping from navigation keys
        default.global.insert(Key { key: KeyCode::Char(' '), modifiers: KeyModifiers::NONE, }.into(), GlobalAction::TogglePause);
        default.navigation.remove(&Key { key: KeyCode::Char(' '), modifiers: KeyModifiers::NONE, }.into());

        let result: KeyConfig = input.try_into().unwrap();

        assert_eq!(result, default);
    }
}
