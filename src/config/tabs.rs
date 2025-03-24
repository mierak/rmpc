#![allow(deprecated)] // TODO remove after cleanup
use std::collections::HashMap;

use anyhow::{Context, Result, ensure};
use derive_more::{Deref, Display, Into};
use itertools::Itertools;
use ratatui::{layout::Direction, style::Color, widgets::Borders};
use serde::{Deserialize, Serialize};

use super::theme::{
    ConfigColor,
    PercentOrLength,
    properties::{Property, PropertyFile, PropertyKind, PropertyKindFile},
};
use crate::shared::id::{self, Id};

#[derive(Debug, Into, Deref, Hash, Eq, PartialEq, Display)]
pub struct TabName(pub std::sync::Arc<String>);
impl From<String> for TabName {
    fn from(value: String) -> Self {
        Self(value.into())
    }
}

impl From<&str> for TabName {
    fn from(value: &str) -> Self {
        Self(value.to_owned().into())
    }
}

impl Clone for TabName {
    fn clone(&self) -> Self {
        TabName(std::sync::Arc::clone(&self.0))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaneTypeFile {
    Queue,
    #[cfg(debug_assertions)]
    Logs,
    Directories,
    Artists,
    Albums,
    AlbumArtists,
    Playlists,
    Search,
    AlbumArt,
    Lyrics,
    ProgressBar,
    Header,
    Tabs,
    TabContent,
    #[cfg(debug_assertions)]
    FrameCount,
    Property {
        content: Vec<PropertyFile<PropertyKindFile>>,
        #[serde(default)]
        align: super::theme::properties::Alignment,
        #[serde(default)]
        scroll_speed: u16,
        #[serde(default)]
        background_color: Option<String>,
    },
    Browser {
        root_tag: String,
        separator: Option<String>,
    },
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, strum::Display, strum::EnumDiscriminants)]
#[strum_discriminants(derive(strum::Display, Hash))]
pub enum PaneType {
    Queue,
    #[cfg(debug_assertions)]
    Logs,
    Directories,
    Artists,
    AlbumArtists,
    Albums,
    Playlists,
    Search,
    AlbumArt,
    Lyrics,
    ProgressBar,
    Header,
    Tabs,
    TabContent,
    #[cfg(debug_assertions)]
    FrameCount,
    Property {
        content: Vec<Property<PropertyKind>>,
        align: ratatui::layout::Alignment,
        scroll_speed: u16,
        background_color: Option<Color>,
    },
    Browser {
        root_tag: String,
        separator: Option<String>,
    },
}

pub const PANES_ALLOWED_IN_BOTH_TAB_AND_LAYOUT: [PaneTypeDiscriminants; 1] =
    [PaneTypeDiscriminants::Property];

#[cfg(debug_assertions)]
pub const UNFOSUSABLE_TABS: [PaneTypeDiscriminants; 8] = [
    PaneTypeDiscriminants::AlbumArt,
    PaneTypeDiscriminants::Lyrics,
    PaneTypeDiscriminants::ProgressBar,
    PaneTypeDiscriminants::Header,
    PaneTypeDiscriminants::Tabs,
    PaneTypeDiscriminants::TabContent,
    PaneTypeDiscriminants::FrameCount,
    PaneTypeDiscriminants::Property,
];

#[cfg(not(debug_assertions))]
pub const UNFOSUSABLE_TABS: [PaneTypeDiscriminants; 7] = [
    PaneTypeDiscriminants::AlbumArt,
    PaneTypeDiscriminants::Lyrics,
    PaneTypeDiscriminants::ProgressBar,
    PaneTypeDiscriminants::Header,
    PaneTypeDiscriminants::Tabs,
    PaneTypeDiscriminants::TabContent,
    PaneTypeDiscriminants::Property,
];

impl Pane {
    pub fn is_focusable(&self) -> bool {
        !UNFOSUSABLE_TABS.contains(&PaneTypeDiscriminants::from(&self.pane))
    }
}

impl TryFrom<PaneTypeFile> for PaneType {
    type Error = anyhow::Error;

    fn try_from(value: PaneTypeFile) -> std::result::Result<Self, Self::Error> {
        Ok(match value {
            PaneTypeFile::Queue => PaneType::Queue,
            #[cfg(debug_assertions)]
            PaneTypeFile::Logs => PaneType::Logs,
            PaneTypeFile::Directories => PaneType::Directories,
            PaneTypeFile::Artists => PaneType::Artists,
            PaneTypeFile::AlbumArtists => PaneType::AlbumArtists,
            PaneTypeFile::Albums => PaneType::Albums,
            PaneTypeFile::Playlists => PaneType::Playlists,
            PaneTypeFile::Search => PaneType::Search,
            PaneTypeFile::AlbumArt => PaneType::AlbumArt,
            PaneTypeFile::Lyrics => PaneType::Lyrics,
            PaneTypeFile::ProgressBar => PaneType::ProgressBar,
            PaneTypeFile::Header => PaneType::Header,
            PaneTypeFile::Tabs => PaneType::Tabs,
            PaneTypeFile::TabContent => PaneType::TabContent,
            #[cfg(debug_assertions)]
            PaneTypeFile::FrameCount => PaneType::FrameCount,
            PaneTypeFile::Property { content, align, scroll_speed, background_color } => {
                PaneType::Property {
                    content: content
                        .into_iter()
                        .map(|prop| prop.try_into().expect(""))
                        .collect_vec(),
                    align: align.into(),
                    scroll_speed,
                    background_color: background_color
                        .as_ref()
                        .map(|s| ConfigColor::try_from(s.as_bytes()))
                        .transpose()
                        .with_context(|| {
                            format!(
                                "Invalid property pane background color: '{background_color:?}'"
                            )
                        })?
                        .map(|c| c.into()),
                }
            }
            PaneTypeFile::Browser { root_tag: tag, separator } => {
                PaneType::Browser { root_tag: tag, separator }
            }
        })
    }
}

impl TryFrom<TabsFile> for Tabs {
    type Error = anyhow::Error;

    fn try_from(value: TabsFile) -> Result<Self, Self::Error> {
        let (names, tabs): (Vec<_>, HashMap<_, _>) = value
            .0
            .into_iter()
            .map(|tab| -> Result<_> {
                Ok(Tab { name: tab.name.into(), panes: tab.pane.convert()? })
            })
            .try_fold((Vec::new(), HashMap::new()), |(mut names, mut tabs), tab| -> Result<_> {
                let tab = tab?;
                names.push(tab.name.clone());
                tabs.insert(tab.name.clone(), tab);
                Ok((names, tabs))
            })?;

        ensure!(!tabs.is_empty(), "At least one tab is required");

        Ok(Self { names, tabs })
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BorderTypeFile {
    Full,
    Single,
    #[default]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct TabsFile(Vec<TabFile>);

#[derive(Debug, Default, Clone)]
pub struct Tabs {
    pub names: Vec<TabName>,
    pub tabs: HashMap<TabName, Tab>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TabFile {
    name: String,
    #[deprecated]
    #[serde(default)]
    border_type: BorderTypeFile,
    pane: PaneOrSplitFile,
}

#[derive(Debug, Clone)]
pub struct Tab {
    pub name: TabName,
    pub panes: SizedPaneOrSplit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DirectionFile {
    Horizontal,
    Vertical,
}

impl From<DirectionFile> for Direction {
    fn from(value: DirectionFile) -> Self {
        match value {
            DirectionFile::Horizontal => Direction::Horizontal,
            DirectionFile::Vertical => Direction::Vertical,
        }
    }
}

impl From<&DirectionFile> for Direction {
    fn from(value: &DirectionFile) -> Self {
        match value {
            DirectionFile::Horizontal => Direction::Horizontal,
            DirectionFile::Vertical => Direction::Vertical,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaneOrSplitFile {
    Pane(PaneTypeFile),
    Split {
        direction: DirectionFile,
        #[serde(default)]
        borders: BordersFile,
        panes: Vec<SubPaneFile>,
    },
}

impl Default for PaneOrSplitFile {
    fn default() -> Self {
        PaneOrSplitFile::Split {
            direction: DirectionFile::Vertical,
            borders: BordersFile::NONE,
            panes: vec![
                SubPaneFile {
                    size: "2".to_string(),
                    borders: BordersFile::NONE,
                    pane: PaneOrSplitFile::Pane(PaneTypeFile::Header),
                },
                SubPaneFile {
                    size: "3".to_string(),
                    borders: BordersFile::NONE,
                    pane: PaneOrSplitFile::Pane(PaneTypeFile::Tabs),
                },
                SubPaneFile {
                    size: "100%".to_string(),
                    borders: BordersFile::NONE,
                    pane: PaneOrSplitFile::Pane(PaneTypeFile::TabContent),
                },
                SubPaneFile {
                    size: "1".to_string(),
                    borders: BordersFile::NONE,
                    pane: PaneOrSplitFile::Pane(PaneTypeFile::ProgressBar),
                },
            ],
        }
    }
}

use bitflags::bitflags;
bitflags! {
    #[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    pub struct BordersFile: u8 {
        const NONE   = 0b0000;
        const TOP    = 0b0001;
        const RIGHT  = 0b0010;
        const BOTTOM = 0b0100;
        const LEFT   = 0b1000;
        const ALL = Self::TOP.bits() | Self::RIGHT.bits() | Self::BOTTOM.bits() | Self::LEFT.bits();
    }
}

impl From<BordersFile> for Borders {
    fn from(value: BordersFile) -> Self {
        self::Borders::from_bits_truncate(value.bits())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubPaneFile {
    pub size: String,
    #[serde(default)]
    pub borders: BordersFile,
    pub pane: PaneOrSplitFile,
}

#[derive(Debug, Clone)]
pub struct Pane {
    pub pane: PaneType,
    pub borders: Borders,
    pub id: Id,
}

#[derive(Debug, Clone)]
pub enum SizedPaneOrSplit {
    Pane(Pane),
    Split { borders: Borders, direction: Direction, panes: Vec<SizedSubPane> },
}

impl Default for SizedPaneOrSplit {
    fn default() -> Self {
        Self::Split { direction: Direction::Horizontal, panes: Vec::new(), borders: Borders::NONE }
    }
}

#[derive(Debug, Clone)]
pub struct SizedSubPane {
    pub size: PercentOrLength,
    pub pane: SizedPaneOrSplit,
}

impl PaneOrSplitFile {
    pub fn convert_recursive(&self, b: Borders) -> Result<SizedPaneOrSplit> {
        Ok(match self {
            PaneOrSplitFile::Pane(pane_type_file) => SizedPaneOrSplit::Pane(Pane {
                pane: pane_type_file.clone().try_into()?,
                borders: b,
                id: id::new(),
            }),
            PaneOrSplitFile::Split { direction, borders, panes } => SizedPaneOrSplit::Split {
                direction: direction.into(),
                borders: Into::<Borders>::into(*borders) | b,
                panes: panes
                    .iter()
                    .map(|sub_pane| -> Result<_> {
                        let borders: Borders = sub_pane.borders.into();
                        let size: PercentOrLength = sub_pane.size.parse()?;
                        let pane = sub_pane.pane.convert_recursive(borders)?;

                        Ok(SizedSubPane { size, pane })
                    })
                    .try_collect()?,
            },
        })
    }

    pub fn convert(&self) -> Result<SizedPaneOrSplit> {
        self.convert_recursive(Borders::NONE)
    }
}

pub struct PaneIter<'a> {
    queue: Vec<&'a SizedPaneOrSplit>,
}

impl<'a> Iterator for PaneIter<'a> {
    type Item = &'a Pane;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Some(SizedPaneOrSplit::Pane(pane)) => Some(pane),
            Some(SizedPaneOrSplit::Split { panes: sub_panes, .. }) => {
                self.queue.extend(sub_panes.iter().map(|v| &v.pane));
                self.next()
            }
            None => None,
        }
    }
}

impl SizedPaneOrSplit {
    pub fn panes_iter(&self) -> PaneIter {
        PaneIter {
            queue: match self {
                p @ SizedPaneOrSplit::Pane { .. } => vec![p],
                SizedPaneOrSplit::Split { panes: sub_panes, .. } => {
                    sub_panes.iter().map(|v| &v.pane).collect()
                }
            },
        }
    }
}

impl Default for TabsFile {
    fn default() -> Self {
        Self(vec![
            TabFile {
                name: "Queue".to_string(),
                border_type: BorderTypeFile::None,
                pane: PaneOrSplitFile::Split {
                    direction: DirectionFile::Horizontal,
                    borders: BordersFile::NONE,
                    panes: vec![
                        SubPaneFile {
                            pane: PaneOrSplitFile::Pane(PaneTypeFile::AlbumArt),
                            size: "40%".to_string(),
                            borders: BordersFile::NONE,
                        },
                        SubPaneFile {
                            pane: PaneOrSplitFile::Pane(PaneTypeFile::Queue),
                            size: "60%".to_string(),
                            borders: BordersFile::NONE,
                        },
                    ],
                },
            },
            #[cfg(debug_assertions)]
            #[cfg(not(test))]
            TabFile {
                name: "Logs".to_string(),
                border_type: BorderTypeFile::None,
                pane: PaneOrSplitFile::Pane(PaneTypeFile::Logs),
            },
            TabFile {
                name: "Directories".to_string(),
                border_type: BorderTypeFile::None,
                pane: PaneOrSplitFile::Pane(PaneTypeFile::Directories),
            },
            TabFile {
                name: "Artists".to_string(),
                border_type: BorderTypeFile::None,
                pane: PaneOrSplitFile::Pane(PaneTypeFile::Artists),
            },
            TabFile {
                name: "Album Artists".to_string(),
                border_type: BorderTypeFile::None,
                pane: PaneOrSplitFile::Pane(PaneTypeFile::AlbumArtists),
            },
            TabFile {
                name: "Albums".to_string(),
                border_type: BorderTypeFile::None,
                pane: PaneOrSplitFile::Pane(PaneTypeFile::Albums),
            },
            TabFile {
                name: "Playlists".to_string(),
                border_type: BorderTypeFile::None,
                pane: PaneOrSplitFile::Pane(PaneTypeFile::Playlists),
            },
            TabFile {
                name: "Search".to_string(),
                border_type: BorderTypeFile::None,
                pane: PaneOrSplitFile::Pane(PaneTypeFile::Search),
            },
        ])
    }
}

pub(crate) fn validate_tabs(layout: &SizedPaneOrSplit, tabs: &Tabs) -> Result<()> {
    let layout_panes = layout.panes_iter().collect_vec();
    ensure!(
        !layout_panes.iter().all(|pane| pane.is_focusable()),
        "Only non-focusable panes are supported in the layout. Possible values: {}",
        UNFOSUSABLE_TABS.iter().join(", ")
    );
    ensure!(
        layout_panes.iter().filter(|pane| pane.pane == PaneType::TabContent).count() == 1,
        "Layout must contain exactly one TabContent pane"
    );

    let all_tab_panes = tabs.tabs.values().flat_map(|tab| tab.panes.panes_iter()).collect_vec();
    let panes_in_both_tabs_and_layout = all_tab_panes
        .iter()
        .flat_map(|tab_pane| {
            layout_panes.iter().filter(|layout_pane| layout_pane.pane == tab_pane.pane)
        })
        .filter(|pane| {
            !PANES_ALLOWED_IN_BOTH_TAB_AND_LAYOUT.contains(&PaneTypeDiscriminants::from(&pane.pane))
        })
        .map(|pane| PaneTypeDiscriminants::from(&pane.pane))
        .unique()
        .collect_vec();
    ensure!(
        panes_in_both_tabs_and_layout.is_empty(),
        "Panes cannot be in layout and tabs at the same time. Please remove following tabs from either layout or tabs: {}",
        panes_in_both_tabs_and_layout.iter().join(", ")
    );

    Ok(())
}
