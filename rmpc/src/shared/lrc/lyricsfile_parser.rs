use std::{path::Path, time::Duration};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::shared::lrc::{Lrc, lyrics::LrcLine};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Lyricsfile {
    version: String,
    metadata: LyricsfileMetadata,
    #[serde(default)]
    lines: Vec<LyricsfileLine>,
    plain: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LyricsfileMetadata {
    title: String,
    artist: String,
    album: Option<String>,
    duration_ms: Option<u64>,
    offset_ms: Option<u64>,
    language: Option<String>,
    #[serde(default)]
    instrumental: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LyricsfileLine {
    text: String,
    start_ms: u64,
    end_ms: Option<u64>,
    #[serde(default)]
    words: Vec<LyricsfileWord>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LyricsfileWord {
    text: String,
    start_ms: u64,
    end_ms: Option<u64>,
}

pub fn parse(path: &Path) -> Result<Lrc> {
    let yaml_content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read lyricsfile at '{}'", path.display()))?;
    let yaml = serde_yaml_ng::from_str::<Lyricsfile>(&yaml_content).with_context(|| {
        format!("Failed to deserialize lyricsfile '{}' at '{}'", yaml_content, path.display())
    })?;

    // Offset handling is not currently specified by the spec, it exists, but
    // has no defined meaning. We do not know whether negative offset means
    // that lyrics should appear earlier or later so ignore it for now.
    let lrc = Lrc {
        title: Some(yaml.metadata.title),
        artist: Some(yaml.metadata.artist),
        album: yaml.metadata.album,
        length: yaml.metadata.duration_ms.map(Duration::from_millis),
        author: None, // Spec does not have author field
        lines: yaml
            .lines
            .into_iter()
            .map(|line| LrcLine { time: Duration::from_millis(line.start_ms), content: line.text })
            .collect(),
    };

    Ok(lrc)
}
