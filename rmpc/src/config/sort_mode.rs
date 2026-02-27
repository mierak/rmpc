use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::{defaults, theme::properties::SongProperty};
use crate::shared::cmp::StringCompare;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortMode {
    Format(Vec<SongProperty>),
    ModifiedTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools, reason = "Bools represent different flags")]
pub struct SortOptions {
    pub mode: SortMode,
    pub group_by_type: bool,
    pub reverse: bool,
    pub ignore_leading_the: bool,
    pub fold_case: bool,
}

impl Default for SortOptions {
    fn default() -> Self {
        Self {
            mode: SortMode::default(),
            group_by_type: true,
            reverse: false,
            ignore_leading_the: false,
            fold_case: false,
        }
    }
}

impl Default for SortMode {
    fn default() -> Self {
        Self::Format(
            defaults::default_song_sort().into_iter().map(SongProperty::from).collect_vec(),
        )
    }
}

impl From<&SortOptions> for StringCompare {
    fn from(value: &SortOptions) -> Self {
        StringCompare::builder()
            .fold_case(value.fold_case)
            .ignore_leading_the(value.ignore_leading_the)
            .build()
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
