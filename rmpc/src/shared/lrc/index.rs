use std::{
    collections::BTreeMap,
    io::BufReader,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result};
use itertools::Itertools;
use rmpc_mpd::commands::Song;
use serde::Serialize;
use unicase::UniCase;
use walkdir::WalkDir;

use crate::shared::{lrc::lyrics::LrcMetadata, macros::try_cont};

/// Index of LRC files for fast song-to-lyrics matching.
///
/// This structure maintains an in-memory index of all LRC files in a directory,
/// allowing for fast lookup of lyrics based on song metadata (artist, title,
/// album). The index is built using efficient metadata-only parsing to avoid
/// processing the entire content of each LRC file during startup.
#[derive(Debug, Default, Serialize)]
pub struct LrcIndex {
    // Using BTreeMap to have a well-defined iteration order.
    index: BTreeMap<PathBuf, LrcMetadata>,
}

impl LrcIndex {
    pub fn index(lyrics_dir: &Path) -> Self {
        let start = std::time::Instant::now();
        let dir = WalkDir::new(lyrics_dir);
        log::info!(dir:?; "Starting lyrics index lyrics");

        let mut index = BTreeMap::new();
        for child in dir {
            let child = try_cont!(child, "skipping child");
            let child = child.path();

            let metadata = try_cont!(Self::index_single(child), "Failed to parse as index entry");

            let Some(metadata) = metadata else {
                log::trace!(child:?; "Entry did not have enough metadata to index, skipping");
                continue;
            };

            log::trace!(child:?; "Successfully indexed lyrics file");

            {
                use std::collections::btree_map::Entry;
                match index.entry(child.to_path_buf()) {
                    Entry::Occupied(mut entry) => {
                        entry.insert(metadata);
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(metadata);
                    }
                }
            }
        }

        log::info!(found_count = index.len(), elapsed:? = start.elapsed(); "Indexed lrc files");
        Self { index }
    }

    pub fn index_single(path: &Path) -> Result<Option<LrcMetadata>> {
        if path.extension().is_none_or(|ext| !ext.to_string_lossy().ends_with("lrc")) {
            log::trace!(path:?; "skipping non lrc file");
            return Ok(None);
        }
        let file = std::fs::File::open(path).context("failed to open a lyrics file")?;

        log::trace!(file:?, path:?; "Trying to index lyrics file");

        LrcMetadata::read(BufReader::new(file)).context("Failed to read a lyrics file")
    }

    fn album_matches(metadata: &LrcMetadata, song_album: Option<&str>) -> bool {
        match (&metadata.album, song_album) {
            (Some(meta_album), Some(song_album)) => {
                UniCase::new(meta_album) == UniCase::new(song_album)
            }
            _ => true,
        }
    }

    fn album_matches_exactly(metadata: &LrcMetadata, song_album: Option<&str>) -> bool {
        match (&metadata.album, song_album) {
            (Some(meta_album), Some(song_album)) => {
                UniCase::new(meta_album) == UniCase::new(song_album)
            }
            _ => false,
        }
    }

    pub(crate) fn find_entry(&self, song: &Song) -> Option<(&Path, &LrcMetadata)> {
        // TODO xxx.last() is called here to not change existing behavior. Consider
        // supporting all the tag entries
        let artist = song.metadata.get("artist").map(|v| v.last())?;
        let title = song.metadata.get("title").map(|v| v.last())?;
        let album = song.metadata.get("album").map(|v| v.last());

        let mut results = self
            .index
            .iter()
            .filter(|(_, metadata)| {
                let artist_matches = metadata
                    .artist
                    .as_ref()
                    .is_some_and(|a| UniCase::new(a) == UniCase::new(artist));
                let title_matches =
                    metadata.title.as_ref().is_some_and(|t| UniCase::new(t) == UniCase::new(title));
                artist_matches && title_matches && Self::album_matches(metadata, album)
            })
            .collect_vec();

        match results[..] {
            [] => {
                log::trace!(artist, title, album; "No Lyrics found for song");
                return None;
            }
            [result] => {
                log::trace!(artist, title, album; "Found exactly one Lyrics entry for song");
                let (path, metadata) = result;
                return Some((path, metadata));
            }
            _ => (),
        }

        log::trace!(artist, title, album; "Found multiple Lyrics entries for song, getting closest match by length");
        let Some(target_duration) = song.duration else {
            // Song does not have a length information, not sure if this can ever happen,
            // but better safe than sorry. Return the first result rather than nothing.
            let (path, metadata) = results[0];
            return Some((path, metadata));
        };

        // Prioritize matches with album
        if results.iter().any(|(_, metadata)| Self::album_matches_exactly(metadata, album)) {
            results.retain(|(_, metadata)| Self::album_matches_exactly(metadata, album));
        }

        let (with_length, without_length): (Vec<_>, Vec<_>) =
            results.into_iter().partition(|(_, e)| e.length.is_some());
        let closest_length_candidate = with_length
            .iter()
            .filter(|(_, metadata)| {
                metadata
                    .length
                    .is_some_and(|l| l.abs_diff(target_duration) < Duration::from_secs(5))
            })
            .min_by_key(|(_, e)| e.length);

        if let Some(&(path, metadata)) = closest_length_candidate {
            // Lrc matching by length was found
            return Some((path, metadata));
        }

        if !without_length.is_empty() {
            // Lrc matching by length was not found, but there are lrc without length
            let (path, metadata) = without_length[0];
            return Some((path, metadata));
        }

        // Lrc with matching length was not found and there are no lrc without
        // length. Return the closest match by length.
        with_length
            .iter()
            .sorted_by(|(_, a), (_, b)| {
                a.length
                    .unwrap_or_default()
                    .abs_diff(target_duration)
                    .cmp(&b.length.unwrap_or_default().abs_diff(target_duration))
            })
            .next()
            .map(|&(p, e)| (p.as_path(), e))
    }

    pub(crate) fn add(&mut self, path: PathBuf, metadata: LrcMetadata) {
        use std::collections::btree_map::Entry;
        match self.index.entry(path) {
            Entry::Occupied(mut entry) => {
                entry.insert(metadata);
            }
            Entry::Vacant(entry) => {
                entry.insert(metadata);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::{
        collections::{BTreeMap, HashMap},
        path::PathBuf,
        time::Duration,
    };

    use bon::builder;
    use chrono::DateTime;
    use rmpc_mpd::commands::{Song, metadata_tag::MetadataTag};

    use super::LrcIndex;
    use crate::shared::lrc::LrcMetadata;

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
            last_modified: DateTime::default(),
            added: Some(DateTime::default()),
        }
    }

    #[builder]
    fn index_entry(
        artist: Option<&str>,
        title: Option<&str>,
        album: Option<&str>,
        author: Option<&str>,
        length: Option<Duration>,
        offset: Option<i64>,
        path: Option<&str>,
    ) -> (PathBuf, LrcMetadata) {
        (path.map(PathBuf::from).unwrap_or_default(), LrcMetadata {
            title: title.map(|v| v.to_owned()),
            artist: artist.map(|v| v.to_owned()),
            album: album.map(|v| v.to_owned()),
            author: author.map(|v| v.to_owned()),
            length,
            offset,
        })
    }

    #[test]
    fn empty_index_matches_nothing() {
        let song = song()
            .artist("123")
            .album("333")
            .title("asdf")
            .duration(Duration::from_secs(147))
            .call();
        let index = LrcIndex { index: BTreeMap::new() };

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
            index: BTreeMap::from_iter(vec![
                index_entry()
                    .artist("123")
                    .album("333")
                    .title("asdf")
                    .length(Duration::from_secs(143))
                    .call(),
            ]),
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
            index: BTreeMap::from_iter(vec![
                index_entry()
                    .artist("aaa")
                    .album("bbb")
                    .title("CCC")
                    .length(Duration::from_secs(143))
                    .call(),
            ]),
        };

        let result = index.find_entry(&song);

        assert!(result.is_some());
    }

    #[test]
    fn song_without_album_matches_lrc_without_album() {
        let song = song().artist("123").title("asdf").duration(Duration::from_secs(147)).call();
        let index = LrcIndex {
            index: BTreeMap::from_iter(vec![
                index_entry().artist("123").title("asdf").length(Duration::from_secs(143)).call(),
            ]),
        };

        let result = index.find_entry(&song);

        assert!(result.is_some());
    }

    #[test]
    fn song_without_album_matches_lrc_with_album() {
        let song = song().artist("123").title("asdf").duration(Duration::from_secs(147)).call();
        let index = LrcIndex {
            index: BTreeMap::from_iter(vec![
                index_entry()
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .length(Duration::from_secs(143))
                    .call(),
            ]),
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
            index: BTreeMap::from_iter(vec![
                index_entry().artist("123").title("asdf").length(Duration::from_secs(143)).call(),
            ]),
        };

        let result = index.find_entry(&song);

        assert!(result.is_some());
    }
    #[test]
    fn length_is_ignored_when_single_match_is_found() {
        let song = song().artist("123").title("asdf").duration(Duration::from_secs(999)).call();
        let index = LrcIndex {
            index: BTreeMap::from_iter(vec![
                index_entry()
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .length(Duration::from_secs(1))
                    .call(),
            ]),
        };

        let result = index.find_entry(&song);

        assert!(result.is_some());
    }

    #[test]
    fn song_has_no_duration() {
        let song = song().artist("123").title("asdf").call();
        let index = LrcIndex {
            index: BTreeMap::from_iter(vec![
                index_entry()
                    .path("1")
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .length(Duration::from_secs(1))
                    .call(),
                index_entry()
                    .path("2")
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .length(Duration::from_secs(1))
                    .call(),
            ]),
        };

        let result = index.find_entry(&song);

        assert!(result.unwrap().0.to_string_lossy() == "1");
    }

    #[test]
    fn length_is_considered_with_multiple_matches() {
        let song = song().artist("123").title("asdf").duration(Duration::from_secs(100)).call();
        let index = LrcIndex {
            index: BTreeMap::from_iter(vec![
                index_entry()
                    .path("should not match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(103))
                    .call(),
                index_entry()
                    .path("should match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(99))
                    .call(),
                index_entry()
                    .path("should not match either")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(108))
                    .call(),
            ]),
        };

        let result = index.find_entry(&song);

        assert!(result.unwrap().0.to_string_lossy() == "should match");
    }

    #[test]
    fn multiple_matches_no_lrc_without_len_no_length_match() {
        let song = song().artist("123").title("asdf").duration(Duration::from_secs(100)).call();
        let index = LrcIndex {
            index: BTreeMap::from_iter(vec![
                index_entry()
                    .path("should not match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(200))
                    .call(),
                index_entry()
                    .path("should match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(199))
                    .call(),
                index_entry()
                    .path("should not match either")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(1))
                    .call(),
            ]),
        };

        let result = index.find_entry(&song);

        assert!(result.unwrap().0.to_string_lossy() == "should match");
    }

    #[test]
    fn both_lrc_with_and_without_len_fallback_to_no_length() {
        let song = song().artist("123").title("asdf").duration(Duration::from_secs(999)).call();
        let index = LrcIndex {
            index: BTreeMap::from_iter(vec![
                index_entry()
                    .path("should not match")
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .length(Duration::from_secs(103))
                    .call(),
                index_entry()
                    .path("no length")
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .call(),
                index_entry()
                    .path("should match")
                    .artist("123")
                    .title("asdf")
                    .album("song does not have me")
                    .length(Duration::from_secs(99))
                    .call(),
            ]),
        };

        let result = index.find_entry(&song);

        assert!(result.unwrap().0.to_string_lossy() == "no length");
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
            index: BTreeMap::from_iter(vec![
                index_entry()
                    .path("should not match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(200))
                    .call(),
                index_entry().path("no album").artist("123").title("asdf").album("456").call(),
                index_entry()
                    .path("should match")
                    .artist("123")
                    .title("asdf")
                    .length(Duration::from_secs(201))
                    .call(),
            ]),
        };

        let result = index.find_entry(&song);

        assert!(result.unwrap().0.to_string_lossy() == "no album");
    }
}
