use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::defaults;
use crate::mpd::mpd_client::FilterKind;

#[derive(Debug, Default, Clone)]
pub struct Search {
    pub case_sensitive: bool,
    pub ignore_diacritics: bool,
    pub search_button: bool,
    pub mode: FilterKind,
    pub tags: Vec<SearchableTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchFile {
    case_sensitive: bool,
    #[serde(default)]
    ignore_diacritics: bool,
    #[serde(default = "defaults::bool::<false>")]
    search_button: bool,
    mode: FilterKindFile,
    tags: Vec<SearchableTagFile>,
}

#[derive(Debug, Default, Clone)]
pub struct SearchableTag {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchableTagFile {
    label: String,
    value: String,
}

impl TryFrom<SearchFile> for Search {
    type Error = anyhow::Error;

    fn try_from(value: SearchFile) -> Result<Self, Self::Error> {
        if value.case_sensitive && value.ignore_diacritics {
            anyhow::bail!(
                "Cannot have both case sensitivity and ignore diacritics enabled at the same time"
            );
        }
        Ok(Self {
            case_sensitive: value.case_sensitive,
            ignore_diacritics: value.ignore_diacritics,
            search_button: value.search_button,
            mode: value.mode.into(),
            tags: if value.tags.is_empty() {
                vec![SearchableTag { label: "Any Tag".to_string(), value: "any".to_string() }]
            } else {
                value
                    .tags
                    .into_iter()
                    .map(|SearchableTagFile { value, label }| SearchableTag { label, value })
                    .collect_vec()
            },
        })
    }
}

impl Default for SearchFile {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            ignore_diacritics: false,
            search_button: false,
            mode: FilterKindFile::Contains,
            tags: [
                SearchableTagFile { value: "any".to_string(), label: "Any Tag".to_string() },
                SearchableTagFile { value: "artist".to_string(), label: "Artist".to_string() },
                SearchableTagFile { value: "album".to_string(), label: "Album".to_string() },
                SearchableTagFile {
                    value: "albumartist".to_string(),
                    label: "Album Artist".to_string(),
                },
                SearchableTagFile { value: "title".to_string(), label: "Title".to_string() },
                SearchableTagFile { value: "filename".to_string(), label: "Filename".to_string() },
                SearchableTagFile { value: "genre".to_string(), label: "Genre".to_string() },
            ]
            .to_vec(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum FilterKindFile {
    Exact,
    StartsWith,
    #[default]
    Contains,
    Regex,
}

impl From<FilterKindFile> for FilterKind {
    fn from(value: FilterKindFile) -> Self {
        match value {
            FilterKindFile::Exact => FilterKind::Exact,
            FilterKindFile::StartsWith => FilterKind::StartsWith,
            FilterKindFile::Contains => FilterKind::Contains,
            FilterKindFile::Regex => FilterKind::Regex,
        }
    }
}
