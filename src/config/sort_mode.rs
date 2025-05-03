use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::{defaults, theme::properties::SongProperty};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortMode {
    Format(Vec<SongProperty>),
    ModifiedTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortOptions {
    pub mode: SortMode,
    pub group_directories_first: bool,
    pub reverse: bool,
}

impl Default for SortOptions {
    fn default() -> Self {
        Self { mode: SortMode::default(), group_directories_first: true, reverse: false }
    }
}

impl Default for SortMode {
    fn default() -> Self {
        Self::Format(
            defaults::default_song_sort().into_iter().map(SongProperty::from).collect_vec(),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub enum SortModeFile {
    Format {
        #[serde(default = "defaults::default_true")]
        group_directories_first: bool,
        #[serde(default = "defaults::default_false")]
        reverse: bool,
    },
    SortFormat {
        #[serde(default = "defaults::default_true")]
        group_directories_first: bool,
        #[serde(default = "defaults::default_false")]
        reverse: bool,
    },
    ModifiedTime {
        #[serde(default = "defaults::default_true")]
        group_directories_first: bool,
        #[serde(default = "defaults::default_false")]
        reverse: bool,
    },
}

impl Default for SortModeFile {
    fn default() -> Self {
        Self::SortFormat { group_directories_first: true, reverse: false }
    }
}
