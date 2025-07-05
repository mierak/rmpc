use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    time::Duration,
};

use anyhow::{Context, Result};
use itertools::Itertools;
use serde::Serialize;
use unicase::UniCase;
use walkdir::WalkDir;

use super::{Lrc, parse_metadata_only};
use crate::{mpd::commands::Song, shared::macros::try_cont};

/// Index of LRC files for fast song-to-lyrics matching.
///
/// This structure maintains an in-memory index of all LRC files in a directory,
/// allowing for fast lookup of lyrics based on song metadata (artist, title,
/// album). The index is built using efficient metadata-only parsing to avoid
/// processing the entire content of each LRC file during startup.
#[derive(Debug, Eq, PartialEq, Default, Serialize)]
pub struct LrcIndex {
    index: Vec<LrcIndexEntry>,
}

/// A single entry in the LRC index containing metadata for fast matching.
///
/// This structure stores the essential metadata needed to match songs to their
/// corresponding LRC files without having to parse the entire file content.
#[derive(Debug, Eq, PartialEq, Hash, Serialize)]
pub struct LrcIndexEntry {
    /// Path to the LRC file
    pub path: PathBuf,
    /// Song title (from [ti:] tag)
    pub title: String,
    /// Artist name (from [ar:] tag)
    pub artist: String,
    /// Album name (from [al:] tag)
    pub album: Option<String>,
    /// Song length (from [length:] tag)
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
        if let Some(entry) = self.find_entry(song) {
            Ok(Some(std::fs::read_to_string(&entry.path)?.parse()?))
        } else {
            Ok(None)
        }
    }

    fn album_matches(entry: &LrcIndexEntry, song_album: Option<&str>) -> bool {
        match (&entry.album, song_album) {
            (Some(entry_album), Some(song_album)) => {
                UniCase::new(entry_album) == UniCase::new(song_album)
            }
            _ => true,
        }
    }

    fn album_matches_exactly(entry: &LrcIndexEntry, song_album: Option<&str>) -> bool {
        match (&entry.album, song_album) {
            (Some(entry_album), Some(song_album)) => {
                UniCase::new(entry_album) == UniCase::new(song_album)
            }
            _ => false,
        }
    }

    fn find_entry(&self, song: &Song) -> Option<&LrcIndexEntry> {
        // TODO xxx.last() is called here to not change existing behavior. Consider
        // supporting all the tag entries
        let artist = song.metadata.get("artist").map(|v| v.last())?;
        let title = song.metadata.get("title").map(|v| v.last())?;
        let album = song.metadata.get("album").map(|v| v.last());

        let mut results = self
            .index
            .iter()
            .filter(|entry| {
                return UniCase::new(&entry.artist) == UniCase::new(artist)
                    && UniCase::new(&entry.title) == UniCase::new(title)
                    && Self::album_matches(entry, album);
            })
            .collect_vec();

        match results.len() {
            0 => {
                log::trace!(artist, title, album; "No Lyrics found for song");
                return None;
            }
            1 => {
                log::trace!(artist, title, album; "Found exactly one Lyrics entry for song");
                Some(results[0])
            }
            _ => {
                log::trace!(artist, title, album; "Found multiple Lyrics entries for song, getting closest match by length");
                if let Some(s_duration) = song.duration {
                    // Prioritize matches with album
                    if results.iter().any(|entry| Self::album_matches_exactly(entry, album)) {
                        results.retain(|entry| Self::album_matches_exactly(entry, album));
                    }

                    let (with_length, without_length): (Vec<_>, Vec<_>) =
                        results.into_iter().partition(|e| e.length.is_some());
                    let entry_with_low_len_diff = with_length
                        .iter()
                        .filter(|entry| {
                            entry
                                .length
                                .is_some_and(|l| l.abs_diff(s_duration) < Duration::from_secs(5))
                        })
                        .min_by_key(|e| e.length);

                    if let Some(entry) = entry_with_low_len_diff {
                        // Lrc matching by length was found
                        Some(entry)
                    } else if !without_length.is_empty() {
                        // Lrc matching by length was not found, but there are lrc without length
                        Some(without_length[0])
                    } else {
                        // Lrc with matching length was not found and there are no lrc without
                        // length. Return the closest match by length.
                        with_length
                            .iter()
                            .sorted_by(|a, b| {
                                a.length
                                    .unwrap_or_default()
                                    .abs_diff(s_duration)
                                    .cmp(&b.length.unwrap_or_default().abs_diff(s_duration))
                            })
                            .next()
                            .copied()
                    }
                } else {
                    // Song does not have a length information, not sure if this can ever happen,
                    // but better safe than sorry. Return the first result rather than nothing.
                    Some(results[0])
                }
            }
        }
    }

    pub(crate) fn add(&mut self, entry: LrcIndexEntry) {
        self.index.push(entry);
    }
}

impl LrcIndexEntry {
    fn read(mut read: impl BufRead, path: PathBuf) -> Result<Option<Self>> {
        let mut content = String::new();

        loop {
            let mut line = String::new();
            if read.read_line(&mut line)? == 0 {
                break; // EOF
            }
            // if this line has a timestamp, stop reading
            // We are looking for lines that start with [ and have a timestamp in them
            // reading all the way to the end of the file is not necessary
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') && trimmed.starts_with('[') {
                if let Some(bracket_end) = trimmed.find(']') {
                    let tag_content = &trimmed[1..bracket_end];
                    if tag_content.chars().next().is_some_and(|c| c.is_numeric())
                        && tag_content.contains(':')
                    {
                        // timestamp found, add this line and stop
                        content.push_str(&line);
                        break;
                    }
                }
            }
            content.push_str(&line);
        }

        let (metadata, _) = parse_metadata_only(&content);

        let Some(artist) = metadata.artist else {
            return Ok(None);
        };
        let Some(title) = metadata.title else {
            return Ok(None);
        };

        Ok(Some(Self { path, title, artist, album: metadata.album, length: metadata.length }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::{collections::HashMap, path::PathBuf, time::Duration};

    use bon::builder;
    use chrono::DateTime;

    use super::{LrcIndex, LrcIndexEntry};
    use crate::mpd::commands::{Song, metadata_tag::MetadataTag};

    #[builder]
    fn song(artist: &str, title: &str, album: Option<&str>, duration: Option<Duration>) -> Song {
        let mut metadata = HashMap::new();
        metadata.insert("artist".into(), MetadataTag::from(artist.to_owned()));
        metadata.insert("title".into(), MetadataTag::from(title.to_owned()));
        if let Some(album) = album {
            metadata.insert("album".into(), MetadataTag::from(album.to_owned()));
        }
        Song {
            id: 0,
            file: String::new(),
            duration,
            metadata,
            stickers: None,
            last_modified: DateTime::default(),
            added: Some(DateTime::default()),
        }
    }

    #[builder]
    fn entry(
        artist: &str,
        title: &str,
        album: Option<&str>,
        length: Option<Duration>,
        path: Option<&str>,
    ) -> LrcIndexEntry {
        LrcIndexEntry {
            path: path.map(PathBuf::from).unwrap_or_default(),
            title: title.to_owned(),
            artist: artist.to_owned(),
            album: album.map(|v| v.to_owned()),
            length,
        }
    }

    #[test]
    fn empty_index_matches_nothing() {
        let song = song()
            .artist("123")
            .album("333")
            .title("asdf")
            .duration(Duration::from_secs(147))
            .call();
        let index = LrcIndex { index: vec![] };

        let result = index.find_entry(&song);

        assert!(result.is_none());
    }

    #[test]
    fn entry_matches_by_album_artist_title_len() {
        let song = song()
            .artist("123")
            .album("333")
            .title("asdf")
            .duration(Duration::from_secs(147))
            .call();
        let index = LrcIndex {
            index: vec![
                entry()
                    .artist("123")
                    .album("333")
                    .title("asdf")
                    .length(Duration::from_secs(143))
                    .call(),
            ],
        };

        let result = index.find_entry(&song);

        assert!(result.is_some());
    }

    #[test]
    fn entry_matches_by_album_artist_title_len_case_insensitive() {
        let song = song()
            .artist("AAA")
            .album("BBB")
            .title("CCC")
            .duration(Duration::from_secs(147))
            .call();
        let index = LrcIndex {
            index: vec![
                entry()
                    .artist("aaa")
                    .album("bbb")
                    .title("CCC")
                    .length(Duration::from_secs(143))
                    .call(),
            ],
        };

        let result = index.find_entry(&song);

        assert!(result.is_some());
    }

    #[test]
    fn song_without_album_matches_lrc_without_album() {
        let song = song().artist("123").title("asdf").duration(Duration::from_secs(147)).call();
        let index = LrcIndex {
            index: vec![
                entry().artist("123").title("asdf").length(Duration::from_secs(143)).call(),
            ],
        };

        let result = index.find_entry(&song);

        assert!(result.is_some());
    }

    #[test]
    fn song_without_album_matches_lrc_with_album() {
        let song = song().artist("123").title("asdf").duration(Duration::from_secs(147)).call();
        let index = LrcIndex {
            index: vec![
                entry()
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .length(Duration::from_secs(143))
                    .call(),
            ],
        };

        let result = index.find_entry(&song);

        assert!(result.is_some());
    }

    #[test]
    fn song_with_album_matches_lrc_without_album() {
        let song = song()
            .artist("123")
            .title("asdf")
            .album("lrc does not have me")
            .duration(Duration::from_secs(147))
            .call();
        let index = LrcIndex {
            index: vec![
                entry().artist("123").title("asdf").length(Duration::from_secs(143)).call(),
            ],
        };

        let result = index.find_entry(&song);

        assert!(result.is_some());
    }
    #[test]
    fn length_is_ignored_when_single_match_is_found() {
        let song = song().artist("123").title("asdf").duration(Duration::from_secs(999)).call();
        let index = LrcIndex {
            index: vec![
                entry()
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .length(Duration::from_secs(1))
                    .call(),
            ],
        };

        let result = index.find_entry(&song);

        assert!(result.is_some());
    }

    #[test]
    fn song_has_no_duration() {
        let song = song().artist("123").title("asdf").call();
        let index = LrcIndex {
            index: vec![
                entry()
                    .path("1")
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .length(Duration::from_secs(1))
                    .call(),
                entry()
                    .path("2")
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .length(Duration::from_secs(1))
                    .call(),
            ],
        };

        let result = index.find_entry(&song);

        assert!(result.unwrap().path.to_string_lossy() == "1");
    }

    #[test]
    fn length_is_considered_with_multiple_matches() {
        let song = song().artist("123").title("asdf").duration(Duration::from_secs(100)).call();
        let index = LrcIndex {
            index: vec![
                entry()
                    .path("should not match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(103))
                    .call(),
                entry()
                    .path("should match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(99))
                    .call(),
                entry()
                    .path("should not match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(108))
                    .call(),
            ],
        };

        let result = index.find_entry(&song);

        assert!(result.unwrap().path.to_string_lossy() == "should match");
    }

    #[test]
    fn multiple_matches_no_lrc_without_len_no_length_match() {
        let song = song().artist("123").title("asdf").duration(Duration::from_secs(100)).call();
        let index = LrcIndex {
            index: vec![
                entry()
                    .path("should not match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(200))
                    .call(),
                entry()
                    .path("should match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(199))
                    .call(),
                entry()
                    .path("should not match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(1))
                    .call(),
            ],
        };

        let result = index.find_entry(&song);

        assert!(result.unwrap().path.to_string_lossy() == "should match");
    }

    #[test]
    fn both_lrc_with_and_without_len_fallback_to_no_length() {
        let song = song().artist("123").title("asdf").duration(Duration::from_secs(999)).call();
        let index = LrcIndex {
            index: vec![
                entry()
                    .path("should not match")
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .length(Duration::from_secs(103))
                    .call(),
                entry()
                    .path("no length")
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .call(),
                entry()
                    .path("should match")
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .length(Duration::from_secs(99))
                    .call(),
            ],
        };

        let result = index.find_entry(&song);

        assert!(result.unwrap().path.to_string_lossy() == "no length");
    }

    #[test]
    fn mutlitle_matches_prioritize_album_match() {
        let song = song()
            .artist("123")
            .album("456")
            .title("asdf")
            .duration(Duration::from_secs(200))
            .call();
        let index = LrcIndex {
            index: vec![
                entry()
                    .path("should not match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(200))
                    .call(),
                entry().path("no album").artist("123").title("asdf").album("456").call(),
                entry()
                    .path("should match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(201))
                    .call(),
            ],
        };

        let result = index.find_entry(&song);

        assert!(result.unwrap().path.to_string_lossy() == "no album");
    }
}
