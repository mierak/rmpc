use std::time::Duration;

use serde::Serialize;

/// A single line of LRC lyrics with its timestamp.
#[derive(Debug, Eq, PartialEq)]
pub struct LrcLine {
    /// The timestamp when this line should be displayed
    pub time: Duration,
    /// The lyrics content for this line
    pub content: String,
}

/// Parsed LRC file containing metadata and timed lyrics lines.
#[derive(Debug, Eq, PartialEq)]
pub struct Lrc {
    /// The timed lyrics lines, sorted by timestamp
    pub lines: Vec<LrcLine>,
    /// Song title (from [ti:] tag)
    pub title: Option<String>,
    /// Artist name (from [ar:] tag)
    pub artist: Option<String>,
    /// Album name (from [al:] tag)
    pub album: Option<String>,
    /// Author/lyricist name (from [au:] tag)
    pub author: Option<String>,
    /// Song length (from [length:] tag)
    pub length: Option<Duration>,
}

/// Metadata extracted from LRC file header tags.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct LrcMetadata {
    /// Song title (from [ti:] tag)
    pub title: Option<String>,
    /// Artist name (from [ar:] tag)
    pub artist: Option<String>,
    /// Album name (from [al:] tag)
    pub album: Option<String>,
    /// Author/lyricist name (from [au:] tag)
    pub author: Option<String>,
    /// Song length (from [length:] tag)
    pub length: Option<Duration>,
    /// Timing offset in milliseconds (from [offset:] tag)
    pub offset: Option<i64>,
}
