use std::collections::HashMap;

use anyhow::{Result, ensure};
use derive_more::{Deref, Display, Into};
use itertools::Itertools;
use ratatui::{layout::Direction, widgets::Borders};
use serde::{Deserialize, Serialize};

use super::{Leak, theme::PercentOrLength};
use crate::shared::id::{self, Id};

#[derive(Debug, Into, Deref, Hash, Eq, PartialEq, Clone, Copy, Display)]
pub struct TabName(pub &'static str);
impl From<String> for TabName {
    fn from(value: String) -> Self {
        Self(value.leak())
    }
}
impl From<&'static str> for TabName {
    fn from(value: &'static str) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord, strum::Display)]
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
}

#[cfg(debug_assertions)]
pub const UNFOSUSABLE_TABS: [PaneType; 7] = [
    PaneType::AlbumArt,
    PaneType::Lyrics,
    PaneType::ProgressBar,
    PaneType::Header,
    PaneType::Tabs,
    PaneType::TabContent,
    PaneType::FrameCount,
];

#[cfg(not(debug_assertions))]
pub const UNFOSUSABLE_TABS: [PaneType; 6] = [
    PaneType::AlbumArt,
    PaneType::Lyrics,
    PaneType::ProgressBar,
    PaneType::Header,
    PaneType::Tabs,
    PaneType::TabContent,
];

impl Pane {
    pub fn is_focusable(&self) -> bool {
        !UNFOSUSABLE_TABS.contains(&self.pane)
    }
}

impl From<&PaneTypeFile> for PaneType {
    fn from(value: &PaneTypeFile) -> Self {
        match value {
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
        }
    }
}

impl TryFrom<TabsFile> for Tabs {
    type Error = anyhow::Error;

    fn try_from(value: TabsFile) -> Result<Self, Self::Error> {
        let (names, tabs): (Vec<_>, HashMap<_, _>) = value
            .0
            .into_iter()
            .map(|tab| -> Result<_> {
                Ok(Tab { name: tab.name.into(), panes: tab.pane.convert(tab.border_type)? })
            })
            .try_fold((Vec::new(), HashMap::new()), |(mut names, mut tabs), tab| -> Result<_> {
                let tab = tab?;
                names.push(tab.name);
                tabs.insert(tab.name, tab.leak());
                Ok((names, tabs))
            })?;

        ensure!(!tabs.is_empty(), "At least one tab is required");

        Ok(Self { tabs, names: names.leak() })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BorderTypeFile {
    Full,
    Single,
    None,
}

#[derive(Debug, Clone, Copy)]
pub enum BorderType {
    Full,
    Single,
    None,
}

impl From<BorderTypeFile> for BorderType {
    fn from(value: BorderTypeFile) -> Self {
        match value {
            BorderTypeFile::Full => BorderType::Full,
            BorderTypeFile::Single => BorderType::Single,
            BorderTypeFile::None => BorderType::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct TabsFile(Vec<TabFile>);

#[derive(Debug, Default, Clone)]
pub struct Tabs {
    pub names: &'static [TabName],
    pub tabs: HashMap<TabName, &'static Tab>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TabFile {
    name: String,
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
    Split { direction: DirectionFile, panes: Vec<SubPaneFile> },
}

impl Default for PaneOrSplitFile {
    fn default() -> Self {
        PaneOrSplitFile::Split {
            direction: DirectionFile::Vertical,
            panes: vec![
                SubPaneFile {
                    size: "2".to_string(),
                    pane: PaneOrSplitFile::Pane(PaneTypeFile::Header),
                },
                SubPaneFile {
                    size: "3".to_string(),
                    pane: PaneOrSplitFile::Pane(PaneTypeFile::Tabs),
                },
                SubPaneFile {
                    size: "100%".to_string(),
                    pane: PaneOrSplitFile::Pane(PaneTypeFile::TabContent),
                },
                SubPaneFile {
                    size: "1".to_string(),
                    pane: PaneOrSplitFile::Pane(PaneTypeFile::ProgressBar),
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubPaneFile {
    pub size: String,
    pub pane: PaneOrSplitFile,
}

#[derive(Debug, Copy, Clone)]
pub struct Pane {
    pub pane: PaneType,
    pub border: Borders,
    pub id: Id,
}

#[derive(Debug, Clone)]
pub enum SizedPaneOrSplit {
    Pane(Pane),
    Split { direction: Direction, panes: Vec<SizedSubPane> },
}

impl Default for SizedPaneOrSplit {
    fn default() -> Self {
        Self::Split { direction: Direction::Horizontal, panes: Vec::new() }
    }
}

#[derive(Debug, Clone)]
pub struct SizedSubPane {
    pub size: PercentOrLength,
    pub pane: SizedPaneOrSplit,
}

impl PaneOrSplitFile {
    pub fn convert(&self, border_type: BorderTypeFile) -> Result<SizedPaneOrSplit> {
        self.convert_recursive(border_type, Borders::NONE)
    }

    fn convert_recursive(
        &self,
        border_type: BorderTypeFile,
        borders: Borders,
    ) -> Result<SizedPaneOrSplit> {
        match self {
            PaneOrSplitFile::Pane(pane) => Ok(SizedPaneOrSplit::Pane(Pane {
                pane: pane.into(),
                border: borders,
                id: id::new(),
            })),
            PaneOrSplitFile::Split { direction, panes: sub_panes } => Ok(SizedPaneOrSplit::Split {
                direction: direction.into(),
                panes: sub_panes
                    .iter()
                    .enumerate()
                    .map(|(idx, sub_pane)| -> Result<_> {
                        let mut size: PercentOrLength = sub_pane.size.parse()?;

                        let borders = match border_type {
                            BorderTypeFile::Full => {
                                if let PercentOrLength::Length(ref mut len) = size {
                                    *len += 2;
                                };
                                Borders::ALL
                            }
                            BorderTypeFile::Single => {
                                let result = match direction {
                                    DirectionFile::Horizontal if idx < sub_panes.len() - 1 => {
                                        Borders::RIGHT | borders
                                    }
                                    DirectionFile::Vertical if idx < sub_panes.len() - 1 => {
                                        Borders::BOTTOM | borders
                                    }
                                    _ => Borders::NONE | borders,
                                };
                                if let PercentOrLength::Length(ref mut len) = size {
                                    match direction {
                                        DirectionFile::Horizontal => {
                                            *len += u16::from(result.contains(Borders::LEFT))
                                                + u16::from(result.contains(Borders::RIGHT));
                                        }
                                        DirectionFile::Vertical => {
                                            *len += u16::from(result.contains(Borders::TOP))
                                                + u16::from(result.contains(Borders::BOTTOM));
                                        }
                                    }
                                };

                                result
                            }
                            BorderTypeFile::None => Borders::NONE,
                        };

                        Ok(SizedSubPane {
                            size,
                            pane: sub_pane.pane.convert_recursive(border_type, borders)?,
                        })
                    })
                    .try_collect()?,
            }),
        }
    }
}

pub struct PaneIter<'a> {
    queue: Vec<&'a SizedPaneOrSplit>,
}

impl Iterator for PaneIter<'_> {
    type Item = Pane;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Some(SizedPaneOrSplit::Pane(pane)) => Some(*pane),
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
                    panes: vec![
                        SubPaneFile {
                            pane: PaneOrSplitFile::Pane(PaneTypeFile::AlbumArt),
                            size: "40%".to_string(),
                        },
                        SubPaneFile {
                            pane: PaneOrSplitFile::Pane(PaneTypeFile::Queue),
                            size: "60%".to_string(),
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
        .collect_vec();
    ensure!(
        panes_in_both_tabs_and_layout.is_empty(),
        "Panes cannot be in layout and tabs at the same time. Please remove following tabs from either layout or tabs: {}",
        panes_in_both_tabs_and_layout.iter().map(|pane| pane.pane).sorted().dedup().join(", ")
    );

    Ok(())
}
