use std::{io::BufRead, io::BufReader, path::PathBuf, time::Duration};

use anyhow::{bail, Context, Result};
use itertools::Itertools;
use serde::Serialize;
use walkdir::WalkDir;

use crate::{mpd::commands::Song, shared::macros::try_cont};

use super::{parse_length, Lrc};
#[derive(Debug, Eq, PartialEq, Default, Serialize)]
pub struct LrcIndex {
    index: Vec<LrcIndexEntry>,
}

impl LrcIndex {
    pub fn index(lyrics_dir: &PathBuf) -> Self {
        let dir = WalkDir::new(lyrics_dir);
        log::debug!(dir:?; "walkdir");

        let mut index = Vec::new();
        for entry in dir {
            let entry = try_cont!(entry, "skipping entry");

            if !entry.file_name().to_string_lossy().ends_with(".lrc") {
                log::debug!(entry:?; "skipping non lrc file");
                continue;
            }

            let file = try_cont!(std::fs::File::open(entry.path()), "failed to open entry file");

            let index_entry = try_cont!(
                LrcIndexEntry::read(BufReader::new(file), entry.path().to_path_buf()),
                "failed to index an entry"
            );

            let Some(index_entry) = index_entry else {
                log::debug!(entry:?; "entry did not have enough metadata to index, skipping");
                continue;
            };

            index.push(index_entry);
        }

        Self { index }
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn find_lrc_for_song(&self, song: &Song) -> Result<Option<Lrc>> {
        match (song.artist(), song.title(), song.album(), song.duration) {
            (Some(artist), Some(title), Some(album), length) => self.find_lrc(artist, title, album, length),
            _ => None,
        }
        .map_or(Ok(None), |lrc| Ok(Some(std::fs::read_to_string(&lrc.path)?.parse()?)))
    }

    fn find_lrc(&self, artist: &str, title: &str, album: &str, length: Option<Duration>) -> Option<&LrcIndexEntry> {
        self.index.iter().find(|entry| {
            log::debug!(entry:?; "searching entry");

            let length_matches = match (entry.length, length) {
                (Some(entry_length), Some(length)) => entry_length.abs_diff(length) < Duration::from_secs(3),
                _ => true,
            };

            length_matches && entry.artist == artist && entry.title == title && entry.album == album
        })
    }
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

impl LrcIndexEntry {
    fn read(mut read: impl BufRead, path: PathBuf) -> Result<Option<Self>> {
        let mut title = None;
        let mut artist = None;
        let mut album = None;
        let mut length = None;

        let mut buf = String::new();
        while read.read_line(&mut buf).is_ok() {
            if buf.trim().is_empty() || buf.starts_with('#') {
                continue;
            }

            let (metadata, rest) = buf
                .trim()
                .strip_prefix('[')
                .and_then(|s| s.split_once(']'))
                .with_context(|| format!("Invalid lrc line format: '{buf}'"))?;
            if !rest.is_empty() {
                break;
            }

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
            buf.clear();
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

        Ok(Some(Self {
            path,
            title,
            artist,
            album,
            length,
        }))
    }
}
