use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use walkdir::WalkDir;

use super::{Lrc, parse_length};
use crate::{mpd::commands::Song, shared::macros::try_cont};

#[derive(Debug, Eq, PartialEq, Default, Serialize)]
pub struct LrcIndex {
    index: Vec<LrcIndexEntry>,
}

#[derive(Debug, Eq, PartialEq, Hash, Serialize)]
pub struct LrcIndexEntry {
    pub path: PathBuf,
    /// ti
    pub title: String,
    /// ar
    pub artist: String,
    /// al
    pub album: String,
    /// length
    pub length: Option<Duration>,
}

impl LrcIndex {
    pub fn index(lyrics_dir: &PathBuf) -> Self {
        let start = std::time::Instant::now();
        let dir = WalkDir::new(lyrics_dir);
        log::info!(dir:?; "Starting lyrics index lyrics");

        let mut index = Vec::new();
        for entry in dir {
            let entry = try_cont!(entry, "skipping entry");

            let index_entry =
                try_cont!(Self::index_single(entry.path().to_path_buf()), "Failed to index entry");

            let Some(index_entry) = index_entry else {
                log::trace!(entry:?; "Entry did not have enough metadata to index, skipping");
                continue;
            };

            log::trace!(entry:?; "Successfully indexed entry");
            index.push(index_entry);
        }

        log::info!(found_count = index.len(), elapsed:? = start.elapsed(); "Indexed lrc files");
        Self { index }
    }

    pub fn index_single(path: PathBuf) -> Result<Option<LrcIndexEntry>> {
        if path.extension().is_none_or(|ext| !ext.to_string_lossy().ends_with("lrc")) {
            log::trace!(path:?; "skipping non lrc file");
            return Ok(None);
        }
        let file = std::fs::File::open(&path).context("failed to open entry file")?;

        log::trace!(file:?, entry:? = path; "Trying to index lyrics entry");

        LrcIndexEntry::read(BufReader::new(file), path).context("Failed to index an entry")
    }

    pub fn find_lrc_for_song(&self, song: &Song) -> Result<Option<Lrc>> {
        match (
            song.metadata.get("artist"),
            song.metadata.get("artist"),
            song.metadata.get("album"),
            song.duration,
        ) {
            (Some(artist), Some(title), Some(album), length) => {
                // TODO xxx.last() is called here to not change existing behavior. Consider
                // supporting all the tag entries
                let lrc_opt = self.find_lrc(artist.last(), title.last(), album.last(), length);
                match lrc_opt {
                    None => log::trace!("No Lyrics found for {:?}", song.metadata),
                    Some(lrc) => log::trace!("Lyrics found at {:?}", lrc.path),
                };
                lrc_opt
            }
            _ => None,
        }
        .map_or(Ok(None), |lrc| Ok(Some(std::fs::read_to_string(&lrc.path)?.parse()?)))
    }

    fn find_lrc(
        &self,
        artist: &str,
        title: &str,
        album: &str,
        length: Option<Duration>,
    ) -> Option<&LrcIndexEntry> {
        log::trace!(
            "Searching Lyrics for song: Title: '{title}', Artist: '{artist}', Album: '{album}', Length: {length:?}"
        );
        self.index.iter().find(|entry| {
            log::trace!(entry:?; "searching entry");

            let length_matches = match (entry.length, length) {
                (Some(entry_length), Some(length)) => {
                    entry_length.abs_diff(length) < Duration::from_secs(3)
                }
                _ => true,
            };

            length_matches && entry.artist == artist && entry.title == title && entry.album == album
        })
    }

    pub(crate) fn add(&mut self, entry: LrcIndexEntry) {
        self.index.push(entry);
    }
}

impl LrcIndexEntry {
    fn read(read: impl BufRead, path: PathBuf) -> Result<Option<Self>> {
        let mut title = None;
        let mut artist = None;
        let mut album = None;
        let mut length = None;

        for buf in read.lines() {
            let buf = buf?;
            if buf.trim().is_empty() || buf.starts_with('#') {
                continue;
            }

            let metadata = buf
                .trim()
                .strip_prefix('[')
                .with_context(|| format!("Invalid lrc line format: '{buf}'"))?;
            if !metadata.ends_with(']') {
                break;
            }
            let metadata = &metadata[..metadata.len() - 1];

            match metadata.chars().next() {
                Some(c) if c.is_numeric() => {
                    break;
                }
                Some(_) => {
                    let (key, value) = metadata
                        .split_once(':')
                        .with_context(|| format!("Invalid metadata line: '{metadata}'"))?;
                    match key.trim() {
                        "ti" => title = Some(value.trim().to_string()),
                        "ar" => artist = Some(value.trim().to_string()),
                        "al" => album = Some(value.trim().to_string()),
                        "length" => length = Some(parse_length(value.trim())?),
                        _ => {}
                    }
                }
                None => {
                    bail!("Invalid lrc metadata/timestamp: '{metadata}'");
                }
            }
        }

        let Some(artist) = artist else {
            return Ok(None);
        };
        let Some(album) = album else {
            return Ok(None);
        };
        let Some(title) = title else {
            return Ok(None);
        };

        Ok(Some(Self { path, title, artist, album, length }))
    }
}
