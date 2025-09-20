use std::{borrow::Cow, cmp::Ordering, str::FromStr};

use crate::{
    config::{
        ShowPlaylistsMode,
        sort_mode::{SortMode, SortOptions},
        theme::{TagResolutionStrategy, properties::SongProperty},
    },
    mpd::commands::{
        Song,
        lsinfo::{Dir, LsInfoEntry},
    },
    shared::cmp::StringCompare,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DirOrSong {
    Dir {
        name: String,
        full_path: String,
        last_modified: chrono::DateTime<chrono::Utc>,
        playlist: bool,
    },
    Song(Song),
}

impl DirOrSong {
    pub fn name_only(name: String) -> Self {
        DirOrSong::Dir {
            name,
            full_path: String::new(),
            last_modified: chrono::Utc::now(),
            playlist: false,
        }
    }

    pub fn playlist_name_only(name: String) -> Self {
        DirOrSong::Dir {
            name,
            full_path: String::new(),
            last_modified: chrono::Utc::now(),
            playlist: true,
        }
    }

    pub fn dir_name_or_file(&self) -> Cow<'_, str> {
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
                let ignore_the = self.opts.ignore_leading_the;
                let mut a_is_leading = true;
                let mut b_is_leading = true;

                for prop in items {
                    let result = CmpByProp::song_cmp(
                        self.song,
                        other.song,
                        prop,
                        self.opts.fold_case,
                        a_is_leading && ignore_the,
                        b_is_leading && ignore_the,
                    );

                    // The property was not empty so we should no longer ignore leading "the"
                    if !result.first_empty {
                        a_is_leading = false;
                    }
                    if !result.second_empty {
                        b_is_leading = false;
                    }

                    if result.ordering != Ordering::Equal {
                        return if self.opts.reverse {
                            result.ordering.reverse()
                        } else {
                            result.ordering
                        };
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
        // If grouping is enabled, we group dirs first, then songs and then
        // playlists
        if self.opts.group_by_type {
            let type_order = match (self.dir_or_song, other.dir_or_song) {
                (DirOrSong::Song(_), DirOrSong::Dir { playlist: true, .. }) => Some(Ordering::Less),
                (DirOrSong::Song(_), DirOrSong::Dir { playlist: false, .. }) => {
                    Some(Ordering::Greater)
                }
                (DirOrSong::Dir { playlist: true, .. }, DirOrSong::Song(_)) => {
                    Some(Ordering::Greater)
                }
                (DirOrSong::Dir { playlist: false, .. }, DirOrSong::Song(_)) => {
                    Some(Ordering::Less)
                }
                (DirOrSong::Dir { playlist: true, .. }, DirOrSong::Dir { playlist: false, .. }) => {
                    Some(Ordering::Greater)
                }
                (DirOrSong::Dir { playlist: false, .. }, DirOrSong::Dir { playlist: true, .. }) => {
                    Some(Ordering::Less)
                }
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
                    StringCompare::from(self.opts).compare(a, b)
                }
                (DirOrSong::Song(a), DirOrSong::Song(b)) => {
                    let ord = a.with_custom_sort(self.opts).cmp(&b.with_custom_sort(self.opts));
                    // have to undo the reverse here because songs' custom sort reverses already
                    if self.opts.reverse { ord.reverse() } else { ord }
                }
                (a @ DirOrSong::Dir { name, .. }, DirOrSong::Song(song))
                | (a @ DirOrSong::Song(song), DirOrSong::Dir { name, .. }) => {
                    let mut is_leading = true;
                    for prop in items {
                        let cmp = StringCompare::builder()
                            .ignore_leading_the(is_leading && self.opts.ignore_leading_the)
                            .fold_case(self.opts.fold_case)
                            .build();

                        let s = song.format(prop, "", TagResolutionStrategy::All);
                        if let Some(s) = s {
                            // The next compare should not ignore leading "the" if the resulting
                            // format is not empty
                            if !s.is_empty() {
                                is_leading = false;
                            }

                            let result = if matches!(a, DirOrSong::Song(..)) {
                                cmp.compare(s.as_ref(), name)
                            } else {
                                cmp.compare(name, s.as_ref())
                            };

                            if result != Ordering::Equal {
                                return if self.opts.reverse { result.reverse() } else { result };
                            }
                        }
                    }
                    Ordering::Greater
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

impl LsInfoEntry {
    pub(crate) fn into_dir_or_song(
        self,
        show_playlists_mode: ShowPlaylistsMode,
    ) -> Option<DirOrSong> {
        match self {
            LsInfoEntry::File(song) => Some(DirOrSong::Song(song)),
            LsInfoEntry::Dir(Dir { name, full_path, last_modified }) => {
                Some(DirOrSong::Dir { name, full_path, last_modified, playlist: false })
            }
            LsInfoEntry::Playlist(playlist) => match show_playlists_mode {
                ShowPlaylistsMode::All => Some(DirOrSong::Dir {
                    name: playlist.name,
                    full_path: playlist.full_path,
                    last_modified: playlist.last_modified,
                    playlist: true,
                }),
                ShowPlaylistsMode::None => None,
                ShowPlaylistsMode::NonRoot if playlist.name == playlist.full_path => None,
                ShowPlaylistsMode::NonRoot => Some(DirOrSong::Dir {
                    name: playlist.name,
                    full_path: playlist.full_path,
                    last_modified: playlist.last_modified,
                    playlist: true,
                }),
            },
        }
    }
}

pub struct CmpByProp {
    pub ordering: Ordering,
    pub first_empty: bool,
    pub second_empty: bool,
}

impl CmpByProp {
    fn opt_str<T: AsRef<str>>(
        a: Option<T>,
        b: Option<T>,
        fold_case: bool,
        a_ignore_leading_the: bool,
        b_ignore_leading_the: bool,
    ) -> Self {
        match (a, b) {
            (Some(a), Some(b)) => {
                let a = a.as_ref();
                let b = b.as_ref();
                Self {
                    ordering: StringCompare::builder()
                        .fold_case(fold_case)
                        .ignore_leading_the_in_a(a_ignore_leading_the)
                        .ignore_leading_the_in_b(b_ignore_leading_the)
                        .build()
                        .compare(a, b),
                    first_empty: a.is_empty(),
                    second_empty: b.is_empty(),
                }
            }
            (_, Some(b)) => Self {
                ordering: Ordering::Greater,
                first_empty: true,
                second_empty: b.as_ref().is_empty(),
            },
            (Some(a), _) => Self {
                ordering: Ordering::Less,
                first_empty: a.as_ref().is_empty(),
                second_empty: true,
            },
            (None, None) => {
                Self { ordering: Ordering::Equal, first_empty: true, second_empty: true }
            }
        }
    }

    fn opt_str_parse<T: AsRef<str>, N: FromStr + Ord>(
        a: Option<T>,
        b: Option<T>,
        fold_case: bool,
        a_ignore_leading_the: bool,
        b_ignore_leading_the: bool,
    ) -> Self {
        match (a, b) {
            (Some(a), Some(b)) => match (a.as_ref().parse::<N>(), b.as_ref().parse::<N>()) {
                (Ok(a), Ok(b)) => {
                    Self { ordering: a.cmp(&b), first_empty: false, second_empty: false }
                }
                _ => Self::opt_str(
                    Some(a),
                    Some(b),
                    fold_case,
                    a_ignore_leading_the,
                    b_ignore_leading_the,
                ),
            },
            (_, Some(b)) => Self {
                ordering: Ordering::Greater,
                first_empty: true,
                second_empty: b.as_ref().is_empty(),
            },
            (Some(a), _) => Self {
                ordering: Ordering::Less,
                first_empty: a.as_ref().is_empty(),
                second_empty: true,
            },
            (None, None) => {
                Self { ordering: Ordering::Equal, first_empty: true, second_empty: true }
            }
        }
    }

    fn cmp<T: Ord>(a: Option<T>, b: Option<T>) -> Self {
        match (a, b) {
            (Some(a), Some(b)) => {
                Self { ordering: a.cmp(&b), first_empty: false, second_empty: false }
            }
            (_, Some(_)) => {
                Self { ordering: Ordering::Greater, first_empty: true, second_empty: false }
            }
            (Some(_), _) => {
                Self { ordering: Ordering::Less, first_empty: false, second_empty: true }
            }
            (None, None) => {
                Self { ordering: Ordering::Equal, first_empty: true, second_empty: true }
            }
        }
    }

    pub fn song_cmp(
        a: &Song,
        b: &Song,
        property: &SongProperty,
        fold_case: bool,
        ignore_the: bool,
        ignore_the_other: bool,
    ) -> CmpByProp {
        match property {
            SongProperty::Filename => CmpByProp::opt_str(
                a.file_name(),
                b.file_name(),
                fold_case,
                ignore_the,
                ignore_the_other,
            ),
            SongProperty::FileExtension => CmpByProp::opt_str(
                a.file_ext(),
                b.file_ext(),
                fold_case,
                ignore_the,
                ignore_the_other,
            ),
            SongProperty::File => CmpByProp::opt_str(
                Some(&a.file),
                Some(&b.file),
                fold_case,
                ignore_the,
                ignore_the_other,
            ),
            SongProperty::Title => CmpByProp::opt_str(
                a.metadata.get("title").map(|v| v.join("")),
                b.metadata.get("title").map(|v| v.join("")),
                fold_case,
                ignore_the,
                ignore_the_other,
            ),
            SongProperty::Artist => CmpByProp::opt_str(
                a.metadata.get("artist").map(|v| v.join("")),
                b.metadata.get("artist").map(|v| v.join("")),
                fold_case,
                ignore_the,
                ignore_the_other,
            ),
            SongProperty::Album => CmpByProp::opt_str(
                a.metadata.get("album").map(|v| v.join("")),
                b.metadata.get("album").map(|v| v.join("")),
                fold_case,
                ignore_the,
                ignore_the_other,
            ),
            SongProperty::Other(prop) => CmpByProp::opt_str(
                a.metadata.get(prop).map(|v| v.join("")),
                b.metadata.get(prop).map(|v| v.join("")),
                fold_case,
                ignore_the,
                ignore_the_other,
            ),
            SongProperty::Track => {
                let self_track = a.metadata.get("track").map(|v| v.join(""));
                let other_track = b.metadata.get("track").map(|v| v.join(""));
                CmpByProp::opt_str_parse::<_, i32>(
                    self_track,
                    other_track,
                    fold_case,
                    ignore_the,
                    ignore_the_other,
                )
            }
            SongProperty::Position => {
                // last() is fine because position should never have multiple values
                let self_pos = a.metadata.get("pos").map(|v| v.last());
                let other_pos = b.metadata.get("pos").map(|v| v.last());
                CmpByProp::opt_str_parse::<_, usize>(
                    self_pos,
                    other_pos,
                    fold_case,
                    ignore_the,
                    ignore_the_other,
                )
            }
            SongProperty::Disc => {
                let self_disc = a.metadata.get("disc").map(|v| v.join(""));
                let other_disc = b.metadata.get("disc").map(|v| v.join(""));
                CmpByProp::opt_str_parse::<_, i32>(
                    self_disc,
                    other_disc,
                    fold_case,
                    ignore_the,
                    ignore_the_other,
                )
            }
            SongProperty::Duration => CmpByProp::cmp(a.duration, b.duration),
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
            playlist: false,
        }
    }

    fn assert_equivalent(actual: &[DirOrSong], expected: &[DirOrSong]) {
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(expected.iter()) {
            assert_eq!(a.dir_name_or_file(), b.dir_name_or_file());
        }
    }

    #[test]
    fn group_dirs_first() {
        let sort = SortOptions {
            mode: SortMode::Format(vec![SongProperty::Title]),
            group_by_type: true,
            reverse: false,
            ignore_leading_the: false,
            fold_case: true,
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
            group_by_type: true,
            reverse: true,
            ignore_leading_the: false,
            fold_case: true,
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
            group_by_type: false,
            reverse: false,
            ignore_leading_the: false,
            fold_case: true,
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
            group_by_type: false,
            reverse: true,
            ignore_leading_the: false,
            fold_case: true,
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
            group_by_type: true,
            reverse: false,
            ignore_leading_the: false,
            fold_case: true,
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
            group_by_type: true,
            reverse: true,
            ignore_leading_the: false,
            fold_case: true,
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
            group_by_type: false,
            reverse: false,
            ignore_leading_the: false,
            fold_case: true,
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
            group_by_type: false,
            reverse: false,
            ignore_leading_the: false,
            fold_case: true,
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
    fn song_sort_ignore_leading_the() {
        let sort = SortOptions {
            mode: SortMode::Format(vec![SongProperty::Title, SongProperty::Artist]),
            group_by_type: false,
            reverse: false,
            ignore_leading_the: true,
            fold_case: true,
        };
        let input = [
            song("a", &[("title", "Gee")]),
            song("b", &[("title", "The b is second")]),
            song("c", &[("title", "a is first")]),
            song("d", &[("title", "the Foo"), ("artist", "b")]),
            song("e", &[("title", "the Foo"), ("artist", "the a")]),
        ];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            song("c", &[]),
            song("b", &[]),
            song("d", &[]),
            song("e", &[]),
            song("a", &[]),
        ]);
    }

    #[test]
    fn song_fold_case() {
        let sort = SortOptions {
            mode: SortMode::Format(vec![SongProperty::File]),
            group_by_type: false,
            reverse: false,
            ignore_leading_the: true,
            fold_case: true,
        };
        let input = [
            song("a", &[]), //
            song("C", &[]),
            song("b", &[]),
            song("d", &[]),
            song("c", &[]),
            song("B", &[]),
            song("G", &[]),
        ];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            song("a", &[]),
            song("b", &[]),
            song("B", &[]),
            song("C", &[]),
            song("c", &[]),
            song("d", &[]),
            song("G", &[]),
        ]);
    }

    #[test]
    fn song_no_fold_case() {
        let sort = SortOptions {
            mode: SortMode::Format(vec![SongProperty::File]),
            group_by_type: false,
            reverse: false,
            ignore_leading_the: true,
            fold_case: false,
        };
        let input = [
            song("a", &[]),
            song("C", &[]),
            song("b", &[]),
            song("d", &[]),
            song("c", &[]),
            song("B", &[]),
            song("G", &[]),
        ];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            song("B", &[]),
            song("C", &[]),
            song("G", &[]),
            song("a", &[]),
            song("b", &[]),
            song("c", &[]),
            song("d", &[]),
        ]);
    }

    #[test]
    fn dir_sort_ignore_leading_the() {
        let sort = SortOptions {
            mode: SortMode::Format(vec![SongProperty::Title, SongProperty::Artist]),
            group_by_type: false,
            reverse: false,
            ignore_leading_the: true,
            fold_case: true,
        };
        let input = [dir("Gee"), dir("The b is second"), dir("a is first"), dir("the Foo")];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            dir("a is first"),
            dir("The b is second"),
            dir("the Foo"),
            dir("Gee"),
        ]);
    }

    #[test]
    fn dir_fold_case() {
        let sort = SortOptions {
            mode: SortMode::Format(vec![SongProperty::File]),
            group_by_type: false,
            reverse: false,
            ignore_leading_the: true,
            fold_case: true,
        };
        let input = [dir("a"), dir("C"), dir("b"), dir("d"), dir("c"), dir("B"), dir("G")];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            dir("a"),
            dir("b"),
            dir("B"),
            dir("C"),
            dir("c"),
            dir("d"),
            dir("G"),
        ]);
    }

    #[test]
    fn dir_no_fold_case() {
        let sort = SortOptions {
            mode: SortMode::Format(vec![SongProperty::File]),
            group_by_type: false,
            reverse: false,
            ignore_leading_the: true,
            fold_case: false,
        };
        let input = [dir("a"), dir("C"), dir("b"), dir("d"), dir("c"), dir("B"), dir("G")];

        let result = input
            .into_iter()
            .sorted_by(|a, b| a.with_custom_sort(&sort).cmp(&b.with_custom_sort(&sort)))
            .collect_vec();

        dbg!(&result);
        assert_equivalent(&result, &[
            dir("B"),
            dir("C"),
            dir("G"),
            dir("a"),
            dir("b"),
            dir("c"),
            dir("d"),
        ]);
    }
}
