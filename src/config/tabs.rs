use anyhow::{bail, ensure, Result};
use derive_more::{Deref, Display, Into};
use itertools::Itertools;
use std::collections::HashMap;

use ratatui::{layout::Direction, widgets::Borders};
use serde::{Deserialize, Serialize};

use crate::shared::{
    geometry::Geometry,
    id::{self, Id},
    percent::Percent,
};

use super::Leak;

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
enum PaneTypeFile {
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
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
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
}

impl PaneTypeFile {
    pub fn is_focusable(self) -> bool {
        !matches!(self, PaneTypeFile::AlbumArt | PaneTypeFile::Lyrics)
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
                Ok(Tab {
                    name: tab.name.into(),
                    panes: tab.pane.convert(tab.border_type)?,
                })
            })
            .try_fold(
                (Vec::new(), HashMap::new()),
                |(mut names, mut tabs), tab| -> Result<_> {
                    let tab = tab?;
                    names.push(tab.name);
                    tabs.insert(tab.name, tab.leak());
                    Ok((names, tabs))
                },
            )?;

        ensure!(!tabs.is_empty(), "At least one tab is required");

        let active_panes = tabs
            .iter()
            .flat_map(|(_, tab)| tab.panes.panes_iter().map(|pane| pane.pane))
            .sorted()
            .dedup()
            .collect_vec()
            .leak();

        Ok(Self {
            tabs,
            names: names.leak(),
            active_panes,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum BorderTypeFile {
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
    pub active_panes: &'static [PaneType],
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
    pub panes: PaneOrSplitWithPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum DirectionFile {
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
enum PaneOrSplitFile {
    Pane(PaneTypeFile),
    Split {
        direction: DirectionFile,
        panes: Vec<SubPaneFile>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SubPaneFile {
    pub size: String,
    pub pane: PaneOrSplitFile,
}

#[derive(Debug, Copy, Clone)]
pub struct Pane {
    pub pane: PaneType,
    pub geometry: Geometry,
    pub border: Borders,
    pub focusable: bool,
    pub id: Id,
}

#[derive(Debug, Clone)]
pub enum PaneOrSplitWithPosition {
    Pane(Pane),
    Split {
        direction: Direction,
        panes: Vec<SubPaneWithPosition>,
    },
}

#[derive(Debug, Clone)]
pub struct SubPaneWithPosition {
    pub size: Percent,
    pub pane: PaneOrSplitWithPosition,
}

impl PaneOrSplitFile {
    fn convert(&self, border_type: BorderTypeFile) -> Result<PaneOrSplitWithPosition> {
        self.convert_recursive(Geometry::new(0, 0, 100, 100), border_type, Borders::NONE)
    }

    fn convert_recursive(
        &self,
        mut available_geometry: Geometry,
        border_type: BorderTypeFile,
        borders: Borders,
    ) -> Result<PaneOrSplitWithPosition> {
        match self {
            PaneOrSplitFile::Pane(pane) => Ok(PaneOrSplitWithPosition::Pane(Pane {
                pane: pane.into(),
                geometry: available_geometry,
                focusable: pane.is_focusable(),
                border: borders,
                id: id::new(),
            })),
            PaneOrSplitFile::Split {
                direction,
                panes: sub_panes,
            } => {
                if sub_panes
                    .iter()
                    .map(|pane| -> Result<_> { Ok(*pane.size.parse::<Percent>()?) })
                    .try_fold(0u16, |acc, val| -> Result<_> {
                        let val = val?;
                        if val == 0 {
                            bail!("Pane size cannot be {}%", val);
                        }
                        Ok(acc + val)
                    })?
                    != 100
                {
                    bail!("Pane sizes in single split must add up to exactly 100%");
                }

                Ok(PaneOrSplitWithPosition::Split {
                    direction: direction.into(),
                    panes: sub_panes
                        .iter()
                        .enumerate()
                        .map(|(idx, sub_pane)| -> Result<_> {
                            let borders = match border_type {
                                BorderTypeFile::Full => Borders::ALL,
                                BorderTypeFile::Single => match direction {
                                    DirectionFile::Horizontal if idx < sub_panes.len() - 1 => Borders::RIGHT,
                                    DirectionFile::Vertical if idx < sub_panes.len() - 1 => Borders::BOTTOM,
                                    _ => Borders::NONE,
                                },
                                BorderTypeFile::None => Borders::NONE,
                            };

                            let size: Percent = sub_pane.size.parse()?;

                            let geo = if idx == sub_panes.len() - 1 {
                                available_geometry.take_remainder()
                            } else {
                                available_geometry.take_chunk(direction.into(), size)
                            };

                            Ok(SubPaneWithPosition {
                                size,
                                pane: sub_pane.pane.convert_recursive(geo, border_type, borders)?,
                            })
                        })
                        .try_collect()?,
                })
            }
        }
    }
}

pub struct PaneIter<'a> {
    queue: Vec<&'a PaneOrSplitWithPosition>,
}

impl Iterator for PaneIter<'_> {
    type Item = Pane;

    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop() {
            Some(PaneOrSplitWithPosition::Pane(pane)) => Some(*pane),
            Some(PaneOrSplitWithPosition::Split { panes: sub_panes, .. }) => {
                self.queue.extend(sub_panes.iter().map(|v| &v.pane));
                self.next()
            }
            None => None,
        }
    }
}

impl PaneOrSplitWithPosition {
    pub fn panes_iter(&self) -> PaneIter {
        PaneIter {
            queue: match self {
                p @ PaneOrSplitWithPosition::Pane { .. } => vec![p],
                PaneOrSplitWithPosition::Split { panes: sub_panes, .. } => sub_panes.iter().map(|v| &v.pane).collect(),
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
