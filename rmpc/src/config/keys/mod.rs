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
use actions::{CommonActionFile, GlobalActionFile, QueueActionsFile};
pub use key::Key;
use serde::{Deserialize, Serialize};

#[cfg(debug_assertions)]
#[cfg(not(debug_assertions))]
#[cfg(debug_assertions)]
use crate::config::keys::actions::CopyContentsFile;
use crate::config::keys::{
    actions::{
        CopyContentFile,
        CopyContentsFile,
        CopyContentsKindFile,
        DuplicateStrategy,
        RateKind,
        SaveKind,
    },
    key::KeySequence,
};

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

// It is important here that the deserialization does not put in filled key maps
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyConfigFile {
    #[serde(default)]
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
        #[cfg(debug_assertions)]
        use LogsActionsFile as L;
        use QueueActionsFile as Q;

        let s = || KeySequence::new();

        let global = HashMap::from([
            (s().char('q'),                       G::Quit),
            (s().char('?'),                       G::ShowHelp),
            (s().char(':'),                       G::CommandMode),
            (s().char('o').char('I'),             G::ShowCurrentSongInfo),
            (s().char('o').char('o'),             G::ShowOutputs),
            (s().char('o').char('p'),             G::ShowDecoders),
            (s().char('o').char('d'),             G::ShowDownloads),
            (s().char('o').char('P'),             G::Partition { name: None, autocreate: false }),
            (s().char('z'),                       G::ToggleRepeat),
            (s().char('x'),                       G::ToggleRandom),
            (s().char('c'),                       G::ToggleConsume),
            (s().char('v'),                       G::ToggleSingle),
            (s().char('p'),                       G::TogglePause),
            (s().char('s'),                       G::Stop),
            (s().char('>'),                       G::NextTrack),
            (s().char('<'),                       G::PreviousTrack),
            (s().char('f'),                       G::SeekForward),
            (s().char('b'),                       G::SeekBack),
            (s().char(','),                       G::VolumeDown),
            (s().char('.'),                       G::VolumeUp),
            (s().tab(),                           G::NextTab),
            (s().char('g').char('t'),             G::NextTab),
            (s().tab().shift(),                   G::PreviousTab),
            (s().char('g').char('T'),             G::PreviousTab),
            (s().char('1'),                       G::SwitchToTab("Queue".to_string())),
            (s().char('2'),                       G::SwitchToTab("Directories".to_string())),
            (s().char('3'),                       G::SwitchToTab("Artists".to_string())),
            (s().char('4'),                       G::SwitchToTab("Album Artists".to_string())),
            (s().char('5'),                       G::SwitchToTab("Albums".to_string())),
            (s().char('6'),                       G::SwitchToTab("Playlists".to_string())),
            (s().char('7'),                       G::SwitchToTab("Search".to_string())),
            (s().char('u'),                       G::Update),
            (s().char('U'),                       G::Rescan),
            (s().char('R'),                       G::AddRandom),
        ]);

        let navigation = HashMap::from([
            (s().esc(),                           C::Close),
            (s().char('c').ctrl(),                C::Close),
            (s().cr(),                            C::Confirm),
            (s().char('k'),                       C::Up),
            (s().up(),                            C::Up),
            (s().char('j'),                       C::Down),
            (s().down(),                          C::Down),
            (s().char('h'),                       C::Left),
            (s().left(),                          C::Left),
            (s().char('l'),                       C::Right),
            (s().right(),                         C::Right),
            (s().char('w').ctrl().char('k'),      C::PaneUp),
            (s().up().ctrl(),                     C::PaneUp),
            (s().char('w').ctrl().char('j'),      C::PaneDown),
            (s().down().ctrl(),                   C::PaneDown),
            (s().char('w').ctrl().char('h'),      C::PaneLeft),
            (s().left().ctrl(),                   C::PaneLeft),
            (s().char('w').ctrl().char('l'),      C::PaneRight),
            (s().right().ctrl(),                  C::PaneRight),
            (s().char('K'),                       C::MoveUp),
            (s().char('J'),                       C::MoveDown),
            (s().char('u').ctrl(),                C::UpHalf),
            (s().char('d').ctrl(),                C::DownHalf),
            (s().char('b').ctrl(),                C::PageUp),
            (s().page_up(),                       C::PageUp),
            (s().char('f').ctrl(),                C::PageDown),
            (s().page_down(),                     C::PageDown),
            (s().char('g').char('g'),             C::Top),
            (s().char('G'),                       C::Bottom),
            (s().char('/'),                       C::EnterSearch),
            (s().char('n'),                       C::NextResult),
            (s().char('N'),                       C::PreviousResult),
            (s().char(' '),                       C::Select),
            (s().char(' ').ctrl(),                C::InvertSelection),
            (s().char('Y'),                       C::CopyToClipboard { kind: CopyContentsKindFile::default() }),
            (s().char('y'),                       C::CopyToClipboard { kind: CopyContentsKindFile::Content(CopyContentsFile { all: false, content: CopyContentFile::DisplayedValue })}),
            (s().char('a'),                       C::Add),
            (s().char('A'),                       C::AddAll),
            (s().char('D'),                       C::Delete),
            (s().char('r').ctrl(),                C::Rename),
            (s().char('i'),                       C::FocusInput),
            (s().char('o').char('i'),             C::ShowInfo),
            (s().char('z').ctrl(),                C::ContextMenu {}),
            (s().char('s').ctrl().char('s'),      C::Save { kind: SaveKind::Modal { all: false, duplicates_strategy: DuplicateStrategy::Ask } }),
            (s().char('s').ctrl().char('a'),      C::Save { kind: SaveKind::Modal { all: true, duplicates_strategy: DuplicateStrategy::Ask } }),
            (s().char('r'),                       C::Rate { kind: RateKind::default(), current: false, min_rating: 0, max_rating: 10 }),
        ]);

        let queue = HashMap::from([
            (s().char('d'),                       Q::Delete),
            (s().char('D'),                       Q::DeleteAll),
            (s().cr(),                            Q::Play),
            (s().char('C'),                       Q::JumpToCurrent),
            (s().char('X'),                       Q::Shuffle),
        ]);

        #[cfg(debug_assertions)]
        let logs = HashMap::from([
            (s().char('D'),                       L::Clear),
            (s().char('S'),                       L::ToggleScroll),
        ]);

        #[cfg(not(debug_assertions))]
        return KeyConfigFile { clear: false, global, navigation, queue };

        #[cfg(debug_assertions)]
        return KeyConfigFile { clear: false, global, navigation, queue, logs };
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
                queue: value
                    .queue
                    .into_iter()
                    .map(|(k, v)| -> anyhow::Result<_> { Ok((k, v.try_into()?)) })
                    .collect::<anyhow::Result<_>>()?,
            })
        } else {
            let global: HashMap<KeySequence, GlobalAction> =
                value.global.into_iter().map(|(k, v)| (k, v.into())).collect();
            let navigation: HashMap<KeySequence, CommonAction> = value
                .navigation
                .into_iter()
                .map(|(k, v)| -> anyhow::Result<_> { Ok((k, v.try_into()?)) })
                .collect::<anyhow::Result<_>>()?;
            let queue: HashMap<KeySequence, QueueActions> = value
                .queue
                .into_iter()
                .map(|(k, v)| -> anyhow::Result<_> { Ok((k, v.try_into()?)) })
                .collect::<anyhow::Result<_>>()?;
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
                                  (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }.into(), QueueActionsFile::JumpToCurrent)]),
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
                                  (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }.into(), QueueActions::JumpToCurrent)]),
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
                                  (Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }.into(), QueueActionsFile::JumpToCurrent)]),
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
        default.queue.insert(Key { key: KeyCode::Char('b'), modifiers: KeyModifiers::SHIFT, }.into(), QueueActions::JumpToCurrent);
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
