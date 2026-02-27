use std::{borrow::Cow, fmt::Write as _};

use strum::Display;

use crate::mpd_client::StrExt;

#[derive(Debug, PartialEq, Eq, Clone, Display)]
#[strum(serialize_all = "lowercase")]
#[allow(unused)]
pub enum Tag {
    Any,
    Artist,
    AlbumArtist,
    Album,
    Title,
    File,
    Genre,
    Custom(String),
}

impl Tag {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Tag::Any => "Any",
            Tag::Artist => "Artist",
            Tag::AlbumArtist => "AlbumArtist",
            Tag::Album => "Album",
            Tag::Title => "Title",
            Tag::File => "File",
            Tag::Genre => "Genre",
            Tag::Custom(v) => v,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FilterKind {
    Exact,
    NotExact,
    StartsWith,
    #[default]
    Contains,
    Regex,
    NotRegex,
    CustomQuery(String),
}

#[derive(Debug)]
pub struct Filter<'value> {
    pub tag: Tag,
    pub value: Cow<'value, str>,
    pub kind: FilterKind,
}

impl From<String> for Tag {
    fn from(value: String) -> Self {
        Self::Custom(value)
    }
}

#[allow(dead_code)]
impl<'value> Filter<'value> {
    pub fn new<T: Into<Tag>, V: Into<Cow<'value, str>>>(tag: T, value: V) -> Self {
        Self { tag: tag.into(), value: value.into(), kind: FilterKind::Exact }
    }

    /// The `tag` and `value` parameters are ignored when the kind is set to
    /// `Custom`.
    pub fn new_with_kind<T: Into<Tag>, V: Into<Cow<'value, str>>>(
        tag: T,
        value: V,
        kind: FilterKind,
    ) -> Self {
        Self { tag: tag.into(), value: value.into(), kind }
    }

    /// The `tag` and `value` parameters are ignored when the kind is set to
    /// `Custom`.
    pub fn with_type(mut self, t: FilterKind) -> Self {
        self.kind = t;
        self
    }

    pub fn to_query_str(&self) -> String {
        match &self.kind {
            FilterKind::Exact => {
                format!("{} == '{}'", self.tag.as_str(), self.value.escape_filter())
            }
            FilterKind::NotExact => {
                format!("{} != '{}'", self.tag.as_str(), self.value.escape_filter())
            }
            FilterKind::StartsWith => {
                format!("{} starts_with '{}'", self.tag.as_str(), self.value.escape_filter())
            }
            FilterKind::Contains => {
                format!("{} contains '{}'", self.tag.as_str(), self.value.escape_filter())
            }
            FilterKind::Regex => {
                format!("{} =~ '{}'", self.tag.as_str(), self.value.escape_filter())
            }
            FilterKind::NotRegex => {
                format!("{} !~ '{}'", self.tag.as_str(), self.value.escape_filter())
            }
            FilterKind::CustomQuery(query) => query.escape_filter(),
        }
    }
}

pub(crate) trait FilterExt {
    fn to_query_str(&self) -> String;
}
impl FilterExt for &[Filter<'_>] {
    fn to_query_str(&self) -> String {
        self.iter().enumerate().fold(String::new(), |mut acc, (idx, filter)| {
            if idx > 0 {
                let _ = write!(acc, " AND ({})", filter.to_query_str());
            } else {
                let _ = write!(acc, "({})", filter.to_query_str());
            }
            acc
        })
    }
}

#[cfg(test)]
mod filter_tests {
    use test_case::test_case;

    use super::*;

    #[test_case(Tag::Artist, "Artist")]
    #[test_case(Tag::Album, "Album")]
    #[test_case(Tag::AlbumArtist, "AlbumArtist")]
    #[test_case(Tag::Title, "Title")]
    #[test_case(Tag::File, "File")]
    #[test_case(Tag::Genre, "Genre")]
    #[test_case(Tag::Custom("customtag".to_string()), "customtag")]
    fn single_value(tag: Tag, expected: &str) {
        let input: &[Filter<'_>] = &[Filter::new(tag, "mrs singer")];

        assert_eq!(input.to_query_str(), format!("({expected} == 'mrs singer')"));
    }

    #[test]
    fn starts_with() {
        let input: &[Filter<'_>] =
            &[Filter::new_with_kind(Tag::Artist, "mrs singer", FilterKind::StartsWith)];

        assert_eq!(input.to_query_str(), "(Artist starts_with 'mrs singer')");
    }

    #[test]
    fn exact() {
        let input: &[Filter<'_>] =
            &[Filter::new_with_kind(Tag::Album, "the greatest", FilterKind::Exact)];

        assert_eq!(input.to_query_str(), "(Album == 'the greatest')");
    }

    #[test]
    fn contains() {
        let input: &[Filter<'_>] =
            &[Filter::new_with_kind(Tag::Album, "the greatest", FilterKind::Contains)];

        assert_eq!(input.to_query_str(), "(Album contains 'the greatest')");
    }

    #[test]
    fn regex() {
        let input: &[Filter<'_>] =
            &[Filter::new_with_kind(Tag::Album, r"the greatest.*\s+[A-Za-z]+$", FilterKind::Regex)];

        assert_eq!(input.to_query_str(), r"(Album =~ 'the greatest.*\\\\s+[A-Za-z]+$')");
    }

    #[test]
    fn multiple_values() {
        let input: &[Filter<'_>] =
            &[Filter::new(Tag::Album, "the greatest"), Filter::new(Tag::Artist, "mrs singer")];

        assert_eq!(input.to_query_str(), "(Album == 'the greatest') AND (Artist == 'mrs singer')");
    }
}
