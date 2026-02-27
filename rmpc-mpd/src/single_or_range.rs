use std::ops::{Range, RangeInclusive};

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct SingleOrRange {
    pub start: usize,
    pub end: Option<usize>,
}

impl From<RangeInclusive<usize>> for SingleOrRange {
    fn from(value: RangeInclusive<usize>) -> Self {
        Self::range(*value.start(), value.end() + 1)
    }
}

impl From<Range<usize>> for SingleOrRange {
    fn from(value: Range<usize>) -> Self {
        Self::range(value.start, value.end)
    }
}

#[allow(dead_code)]
impl SingleOrRange {
    pub fn single(idx: usize) -> Self {
        Self { start: idx, end: None }
    }

    pub fn range(start: usize, end: usize) -> Self {
        Self { start, end: Some(end) }
    }

    pub fn as_mpd_range(&self) -> String {
        if let Some(end) = self.end {
            format!("\"{}:{}\"", self.start, end)
        } else {
            format!("\"{}\"", self.start)
        }
    }
}
