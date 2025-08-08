use std::borrow::Cow;

use serde::Serialize;

#[derive(Debug, Serialize, PartialEq, Eq, Clone, Ord, PartialOrd)]
#[serde(untagged)]
/// Either a single or multiple tags. Extra care must be taken to never
/// construct the [`MetadataTag::Multiple`] variant with no items as that would
/// make [last] and [`last_mut`] panic.
pub enum MetadataTag {
    Single(String),
    Multiple(Vec<String>),
}

impl From<String> for MetadataTag {
    fn from(value: String) -> Self {
        MetadataTag::Single(value)
    }
}

impl From<Vec<String>> for MetadataTag {
    fn from(value: Vec<String>) -> Self {
        MetadataTag::Multiple(value)
    }
}

impl From<&str> for MetadataTag {
    fn from(value: &str) -> Self {
        MetadataTag::Single(value.to_owned())
    }
}

#[allow(unused)]
pub trait MetadataTagExt<'s> {
    fn last(&self) -> Option<&str>;
    fn join(&'s self, separator: &str) -> Option<Cow<'s, str>>;
}

impl MetadataTag {
    pub fn first(&self) -> &str {
        match self {
            MetadataTag::Single(v) => v.as_str(),
            MetadataTag::Multiple(items) => items
                .first()
                .map(|s| s.as_str())
                .expect("Multiple tags to contain at least one value"),
        }
    }

    pub fn last(&self) -> &str {
        match self {
            MetadataTag::Single(v) => v.as_str(),
            MetadataTag::Multiple(items) => items
                .last()
                .map(|s| s.as_str())
                .expect("Multiple tags to contain at least one value"),
        }
    }

    pub fn nth(&self, idx: usize) -> &str {
        match self {
            MetadataTag::Single(v) => v.as_str(),
            MetadataTag::Multiple(items) => {
                if idx >= items.len() { items.last() } else { items.get(idx) }
                    .map(|s| s.as_str())
                    .expect("Multiple tags to contain at least one value")
            }
        }
    }

    pub fn last_mut(&mut self) -> &mut String {
        match self {
            MetadataTag::Single(v) => v,
            MetadataTag::Multiple(items) => {
                items.last_mut().expect("Multiple tags to contain at least one value")
            }
        }
    }

    pub fn join<'s>(&'s self, separator: &str) -> Cow<'s, str> {
        match self {
            MetadataTag::Single(v) => Cow::Borrowed(v),
            MetadataTag::Multiple(items) => Cow::Owned(items.join(separator)),
        }
    }

    pub fn for_each(&self, mut cb: impl FnMut(&str)) {
        match self {
            MetadataTag::Single(item) => (cb)(item),
            MetadataTag::Multiple(items) => {
                for item in items {
                    (cb)(item);
                }
            }
        }
    }

    pub fn iter(&self) -> MetadataTagIterator<'_> {
        MetadataTagIterator { inner: self, current: 0 }
    }
}

pub struct MetadataTagIterator<'a> {
    inner: &'a MetadataTag,
    current: usize,
}

impl<'a> Iterator for MetadataTagIterator<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let res = match self.inner {
            MetadataTag::Single(item) if self.current == 0 => Some(item.as_str()),
            MetadataTag::Single(_item) => None,
            MetadataTag::Multiple(items) => items.get(self.current).map(|v| v.as_str()),
        };
        self.current += 1;
        res
    }
}

impl<'s> MetadataTagExt<'s> for Option<&'s MetadataTag> {
    fn last(&self) -> Option<&str> {
        self.map(|s| s.last())
    }

    fn join(&'s self, separator: &str) -> Option<Cow<'s, str>> {
        self.map(|s| s.join(separator))
    }
}
