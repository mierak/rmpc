use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::mpd::mpd_client::FilterKind;

#[derive(Debug, Default, Clone)]
pub struct Search {
    pub case_sensitive: bool,
    pub mode: FilterKind,
    pub searchable_tags: &'static [SearchableTag],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchFile {
    case_sensitive: bool,
    mode: FilterKindFile,
    searchable_tags: Vec<SearchableTagFile>,
}

#[derive(Debug, Default, Clone)]
pub struct SearchableTag {
    pub label: &'static str,
    pub tag: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SearchableTagFile {
    label: String,
    tag: String,
}

impl From<SearchFile> for Search {
    fn from(value: SearchFile) -> Self {
        Self {
            case_sensitive: value.case_sensitive,
            mode: value.mode.into(),
            searchable_tags: if value.searchable_tags.is_empty() {
                vec![SearchableTag {
                    label: "Any Tag",
                    tag: "any",
                }]
            } else {
                value
                    .searchable_tags
                    .into_iter()
                    .map(|SearchableTagFile { tag, label }| SearchableTag {
                        tag: tag.leak(),
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
            searchable_tags: [
                SearchableTagFile {
                    tag: "any".to_string(),
                    label: "Any Tag".to_string(),
                },
                SearchableTagFile {
                    tag: "artist".to_string(),
                    label: "Artist".to_string(),
                },
                SearchableTagFile {
                    tag: "album".to_string(),
                    label: "Album".to_string(),
                },
                SearchableTagFile {
                    tag: "albumartist".to_string(),
                    label: "Album Artist".to_string(),
                },
                SearchableTagFile {
                    tag: "title".to_string(),
                    label: "Title".to_string(),
                },
                SearchableTagFile {
                    tag: "filename".to_string(),
                    label: "Filename".to_string(),
                },
                SearchableTagFile {
                    tag: "genre".to_string(),
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
