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
    pub group_by_type: bool,
    pub reverse: bool,
}

impl Default for SortOptions {
    fn default() -> Self {
        Self { mode: SortMode::default(), group_by_type: true, reverse: false }
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
        #[serde(default = "defaults::bool::<true>")]
        group_by_type: bool,
        #[serde(default = "defaults::bool::<false>")]
        reverse: bool,
    },
    SortFormat {
        #[serde(default = "defaults::bool::<true>")]
        group_by_type: bool,
        #[serde(default = "defaults::bool::<false>")]
        reverse: bool,
    },
    ModifiedTime {
        #[serde(default = "defaults::bool::<true>")]
        group_by_type: bool,
        #[serde(default = "defaults::bool::<false>")]
        reverse: bool,
    },
}

impl Default for SortModeFile {
    fn default() -> Self {
        Self::SortFormat { group_by_type: true, reverse: false }
    }
}
