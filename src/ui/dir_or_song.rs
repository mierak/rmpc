use std::{borrow::Cow, cmp::Ordering};

use unicase::UniCase;

use crate::{
    config::sort_mode::{SortMode, SortOptions},
    mpd::commands::{Song, lsinfo::LsInfoEntry},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DirOrSong {
    Dir { name: String, full_path: String, last_modified: chrono::DateTime<chrono::Utc> },
    Song(Song),
}

impl DirOrSong {
    pub fn name_only(name: String) -> Self {
        DirOrSong::Dir { name, full_path: String::new(), last_modified: chrono::Utc::now() }
    }

    pub fn dir_name_or_file_name(&self) -> Cow<str> {
        match self {
            DirOrSong::Dir { name, .. } => Cow::Borrowed(name),
            DirOrSong::Song(song) => Cow::Borrowed(&song.file),
        }
    }

    pub fn last_modified(&self) -> chrono::DateTime<chrono::Utc> {
        match self {
            DirOrSong::Dir { last_modified, .. } => *last_modified,
            DirOrSong::Song(song) => song.last_modified,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SongCustomSort<'a, 'opts> {
    song: &'a Song,
    opts: &'opts SortOptions,
}

impl Song {
    pub(crate) fn with_custom_sort<'song, 'opts>(
        &'song self,
        opts: &'opts SortOptions,
    ) -> SongCustomSort<'song, 'opts> {
        SongCustomSort { song: self, opts }
    }
}

impl Ord for SongCustomSort<'_, '_> {
    fn cmp(&self, other: &Self) -> Ordering {
        match &self.opts.mode {
            SortMode::Format(items) => {
                for prop in items {
                    let ord = self.song.cmp_by_prop(other.song, prop);
                    if ord != Ordering::Equal {
                        return if self.opts.reverse { ord.reverse() } else { ord };
                    }
                }
                Ordering::Equal
            }
            SortMode::ModifiedTime => {
                let result = self.song.last_modified.cmp(&other.song.last_modified);
                if self.opts.reverse { result.reverse() } else { result }
            }
        }
    }
}

impl PartialOrd for SongCustomSort<'_, '_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DirOrSongCustomSort<'dirsong, 'opts> {
    dir_or_song: &'dirsong DirOrSong,
    opts: &'opts SortOptions,
}

impl DirOrSong {
    pub(crate) fn with_custom_sort<'dirsong, 'opts>(
        &'dirsong self,
        opts: &'opts SortOptions,
    ) -> DirOrSongCustomSort<'dirsong, 'opts> {
        DirOrSongCustomSort { dir_or_song: self, opts }
    }
}

impl Ord for DirOrSongCustomSort<'_, '_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // dirs go first if group_directories_first is true
        if self.opts.group_directories_first {
            let type_order = match (self.dir_or_song, other.dir_or_song) {
                (DirOrSong::Song(_), DirOrSong::Dir { .. }) => Some(Ordering::Greater),
                (DirOrSong::Dir { .. }, DirOrSong::Song(_)) => Some(Ordering::Less),
                _ => None,
            };

            if let Some(order) = type_order {
                return if self.opts.reverse { order.reverse() } else { order };
            }
        }

        let order = match &self.opts.mode {
            // directory grouping was already accounted above so we can now naively sort by
            // mtime
            SortMode::ModifiedTime => {
                self.dir_or_song.last_modified().cmp(&other.dir_or_song.last_modified())
            }
            // compare dirs against dirs, dirs against songs and songs against songs
            SortMode::Format(items) => match (self.dir_or_song, other.dir_or_song) {
                (DirOrSong::Dir { name: a, .. }, DirOrSong::Dir { name: b, .. }) => {
                    UniCase::new(a).cmp(&UniCase::new(b))
                }
                (DirOrSong::Song(a), DirOrSong::Song(b)) => {
                    let ord = a.with_custom_sort(self.opts).cmp(&b.with_custom_sort(self.opts));
                    // have to undo the reverse here because songs' custom sort reverses already
                    if self.opts.reverse { ord.reverse() } else { ord }
                }
                (DirOrSong::Song(song), DirOrSong::Dir { name, .. }) => {
                    for prop in items {
                        let s = song.format(prop, "");
                        if let Some(s) = s {
                            let result = UniCase::new(s.as_ref()).cmp(&UniCase::new(name));
                            if result != Ordering::Equal {
                                return if self.opts.reverse { result.reverse() } else { result };
                            }
                        }
                    }
                    Ordering::Greater
                }
                (DirOrSong::Dir { name, .. }, DirOrSong::Song(song)) => {
                    for prop in items {
                        let s = song.format(prop, "");
                        if let Some(s) = s {
                            let result = UniCase::new(name.as_str()).cmp(&UniCase::new(s.as_ref()));
                            if result != Ordering::Equal {
                                return if self.opts.reverse { result.reverse() } else { result };
                            }
                        }
                    }
                    Ordering::Less
                }
            },
        };
        return if self.opts.reverse { order.reverse() } else { order };
    }
}

impl PartialOrd for DirOrSongCustomSort<'_, '_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<LsInfoEntry> for Option<DirOrSong> {
    fn from(value: LsInfoEntry) -> Self {
        match value {
            LsInfoEntry::Dir(crate::mpd::commands::lsinfo::Dir {
                path,
                full_path,
                last_modified,
            }) => Some(DirOrSong::Dir { name: path, full_path, last_modified }),
            LsInfoEntry::File(song) => Some(DirOrSong::Song(song)),
            LsInfoEntry::Playlist(_) => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod ordtest {
    use std::{
        sync::{LazyLock, atomic::AtomicU32},
        time::Duration,
    };

    use itertools::Itertools;

    use super::DirOrSong;
    use crate::{
        config::{
            sort_mode::{SortMode, SortOptions},
            theme::properties::SongProperty,
        },
        mpd::commands::{Song, metadata_tag::MetadataTag},
    };

    static LAST_ID: AtomicU32 = AtomicU32::new(1);
    static NOW: LazyLock<chrono::DateTime<chrono::Utc>> = LazyLock::new(chrono::Utc::now);
    pub fn new_id() -> u32 {
        LAST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    fn song(name: &str, metadata: &[(&str, &str)]) -> DirOrSong {
        song_mtime(name, metadata, &NOW.to_rfc3339())
    }

    fn song_mtime(name: &str, metadata: &[(&str, &str)], mtime: &str) -> DirOrSong {
        DirOrSong::Song(Song {
            id: new_id(),
            file: name.to_string(),
            duration: Some(Duration::from_secs(1)),
            metadata: metadata
                .iter()
                .map(|(k, v)| ((*k).to_string(), MetadataTag::Single((*v).to_string())))
                .collect(),
            stickers: None,
            last_modified: mtime.parse().unwrap(),
            added: None,
        })
    }

    fn dir(name: &str) -> DirOrSong {
        dir_mtime(name, &NOW.to_rfc3339())
    }
    fn dir_mtime(name: &str, mtime: &str) -> DirOrSong {
        DirOrSong::Dir {
            name: name.to_string(),
            full_path: name.to_string(),
            last_modified: mtime.parse().unwrap(),
        }
    }

    fn assert_equivalent(actual: &[DirOrSong], expected: &[DirOrSong]) {
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(expected.iter()) {
            assert_eq!(a.dir_name_or_file_name(), b.dir_name_or_file_name());
        }
    }

    #[test]
    fn group_dirs_first() {
        let sort = SortOptions {
            mode: SortMode::Format(vec![SongProperty::Title]),
            group_directories_first: true,
            reverse: false,
        };
        let input = [
            song("song3", &[("title", "c")]),
            dir("dir1"),
            song("song2", &[("title", "b")]),
            dir("dir3"),
            song("song1", &[("title", "a")]),
            dir("dir2"),
        ];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            dir("dir1"),
            dir("dir2"),
            dir("dir3"),
            song("song1", &[]),
            song("song2", &[]),
            song("song3", &[]),
        ]);
    }

    #[test]
    fn group_dirs_first_reversed() {
        let sort = SortOptions {
            mode: SortMode::Format(vec![SongProperty::Title]),
            group_directories_first: true,
            reverse: true,
        };
        let input = [
            song("song3", &[("title", "c")]),
            dir("dir1"),
            song("song2", &[("title", "b")]),
            dir("dir3"),
            song("song1", &[("title", "a")]),
            dir("dir2"),
        ];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            song("song3", &[]),
            song("song2", &[]),
            song("song1", &[]),
            dir("dir3"),
            dir("dir2"),
            dir("dir1"),
        ]);
    }

    #[test]
    fn no_grouping() {
        let sort = SortOptions {
            mode: SortMode::Format(vec![SongProperty::Title]),
            group_directories_first: false,
            reverse: false,
        };
        let input = [
            song("song3", &[("title", "e")]),
            song("song2", &[("title", "c")]),
            dir("b_dir1"),
            dir("f_dir3"),
            song("song1", &[("title", "a")]),
            dir("d_dir2"),
        ];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            song("song1", &[]),
            dir("b_dir1"),
            song("song2", &[]),
            dir("d_dir2"),
            song("song3", &[]),
            dir("f_dir3"),
        ]);
    }

    #[test]
    fn no_grouping_reversed() {
        let sort = SortOptions {
            mode: SortMode::Format(vec![SongProperty::Title]),
            group_directories_first: false,
            reverse: true,
        };
        let input = [
            song("e", &[("title", "e")]),
            song("c", &[("title", "c")]),
            dir("b"),
            dir("f"),
            song("a", &[("title", "a")]),
            dir("d"),
        ];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            dir("f"),
            song("e", &[]),
            dir("d"),
            song("c", &[]),
            dir("b"),
            song("a", &[]),
        ]);
    }

    #[test]
    fn group_dirs_mtime() {
        let sort = SortOptions {
            mode: SortMode::ModifiedTime,
            group_directories_first: true,
            reverse: false,
        };
        let input = [
            song_mtime("e", &[], "2025-04-02T14:52:05Z"),
            song_mtime("c", &[], "2025-04-02T14:52:03Z"),
            dir_mtime("b", "2025-04-02T14:52:02Z"),
            dir_mtime("f", "2025-04-02T14:52:06Z"),
            song_mtime("a", &[], "2025-04-02T14:52:01Z"),
            dir_mtime("d", "2025-04-02T14:52:04Z"),
        ];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            dir("b"),
            dir("d"),
            dir("f"),
            song("a", &[]),
            song("c", &[]),
            song("e", &[]),
        ]);
    }

    #[test]
    fn group_dirs_mtime_reversed() {
        let sort = SortOptions {
            mode: SortMode::ModifiedTime,
            group_directories_first: true,
            reverse: true,
        };
        let input = [
            song_mtime("e", &[], "2025-04-02T14:52:05Z"),
            song_mtime("c", &[], "2025-04-02T14:52:03Z"),
            dir_mtime("b", "2025-04-02T14:52:02Z"),
            dir_mtime("f", "2025-04-02T14:52:06Z"),
            song_mtime("a", &[], "2025-04-02T14:52:01Z"),
            dir_mtime("d", "2025-04-02T14:52:04Z"),
        ];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            song("e", &[]),
            song("c", &[]),
            song("a", &[]),
            dir("f"),
            dir("d"),
            dir("b"),
        ]);
    }

    #[test]
    fn no_grouping_dirs_mtime() {
        let sort = SortOptions {
            mode: SortMode::ModifiedTime,
            group_directories_first: false,
            reverse: false,
        };
        let input = [
            song_mtime("e", &[], "2025-04-02T14:52:05Z"),
            song_mtime("c", &[], "2025-04-02T14:52:03Z"),
            dir_mtime("b", "2025-04-02T14:52:02Z"),
            dir_mtime("f", "2025-04-02T14:52:06Z"),
            song_mtime("a", &[], "2025-04-02T14:52:01Z"),
            dir_mtime("d", "2025-04-02T14:52:04Z"),
        ];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            song("a", &[]),
            dir("b"),
            song("c", &[]),
            dir("d"),
            song("e", &[]),
            dir("f"),
        ]);
    }

    #[test]
    fn no_grouping_dirs_mtime_reversed() {
        let sort = SortOptions {
            mode: SortMode::ModifiedTime,
            group_directories_first: false,
            reverse: false,
        };
        let input = [
            song_mtime("e", &[], "2025-04-02T14:52:05Z"),
            song_mtime("c", &[], "2025-04-02T14:52:03Z"),
            dir_mtime("b", "2025-04-02T14:52:02Z"),
            dir_mtime("f", "2025-04-02T14:52:06Z"),
            song_mtime("a", &[], "2025-04-02T14:52:01Z"),
            dir_mtime("d", "2025-04-02T14:52:04Z"),
        ];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            song("a", &[]),
            dir("b"),
            song("c", &[]),
            dir("d"),
            song("e", &[]),
            dir("f"),
        ]);
    }
}
