use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::mpd::mpd_client::FilterKind;

#[derive(Debug, Default, Clone)]
pub struct Search {
    pub case_sensitive: bool,
    pub mode: FilterKind,
    pub tags: &'static [SearchableTag],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchFile {
    case_sensitive: bool,
    mode: FilterKindFile,
    tags: Vec<SearchableTagFile>,
}

#[derive(Debug, Default, Clone)]
pub struct SearchableTag {
    pub label: &'static str,
    pub value: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchableTagFile {
    label: String,
    value: String,
}

impl From<SearchFile> for Search {
    fn from(value: SearchFile) -> Self {
        Self {
            case_sensitive: value.case_sensitive,
            mode: value.mode.into(),
            tags: if value.tags.is_empty() {
                vec![SearchableTag {
                    label: "Any Tag",
                    value: "any",
                }]
            } else {
                value
                    .tags
                    .into_iter()
                    .map(|SearchableTagFile { value: tag, label }| SearchableTag {
                        value: tag.leak(),
                        label: label.leak(),
                    })
                    .collect_vec()
            }
            .leak(),
        }
    }
}

impl Default for SearchFile {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            mode: FilterKindFile::Contains,
            tags: [
                SearchableTagFile {
                    value: "any".to_string(),
                    label: "Any Tag".to_string(),
                },
                SearchableTagFile {
                    value: "artist".to_string(),
                    label: "Artist".to_string(),
                },
                SearchableTagFile {
                    value: "album".to_string(),
                    label: "Album".to_string(),
                },
                SearchableTagFile {
                    value: "albumartist".to_string(),
                    label: "Album Artist".to_string(),
                },
                SearchableTagFile {
                    value: "title".to_string(),
                    label: "Title".to_string(),
                },
                SearchableTagFile {
                    value: "filename".to_string(),
                    label: "Filename".to_string(),
                },
                SearchableTagFile {
                    value: "genre".to_string(),
                    label: "Genre".to_string(),
                },
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
