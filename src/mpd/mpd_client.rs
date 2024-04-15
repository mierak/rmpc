use std::collections::BTreeSet;

use anyhow::Result;
use derive_more::Deref;
use strum::{AsRefStr, Display};

use crate::utils::macros::status_error;

use super::{
    client::Client,
    commands::{
        list::MpdList, list_playlist::FileList, status::OnOffOneshot, volume::Bound, IdleEvent, ListFiles, LsInfo,
        Playlist, Song, Status, Volume,
    },
    errors::{ErrorCode, MpdError, MpdFailureResponse},
    proto_client::ProtoClient,
    version::Version,
};

type MpdResult<T> = Result<T, MpdError>;

#[derive(AsRefStr, Debug)]
#[allow(dead_code)]
pub enum SaveMode {
    #[strum(serialize = "create")]
    Create,
    #[strum(serialize = "append")]
    Append,
    #[strum(serialize = "replace")]
    Replace,
}

pub trait MpdClient {
    fn idle(&mut self) -> MpdResult<Vec<IdleEvent>>;
    fn get_volume(&mut self) -> MpdResult<Volume>;
    fn set_volume(&mut self, volume: Volume) -> MpdResult<()>;
    fn get_current_song(&mut self) -> MpdResult<Option<Song>>;
    fn get_status(&mut self) -> MpdResult<Status>;
    // Playback control
    fn pause_toggle(&mut self) -> MpdResult<()>;
    fn next(&mut self) -> MpdResult<()>;
    fn prev(&mut self) -> MpdResult<()>;
    fn play_pos(&mut self, pos: u32) -> MpdResult<()>;
    fn play(&mut self) -> MpdResult<()>;
    fn play_id(&mut self, id: u32) -> MpdResult<()>;
    fn stop(&mut self) -> MpdResult<()>;
    fn seek_curr_forwards(&mut self, time_sec: u32) -> MpdResult<()>;
    fn seek_curr_backwards(&mut self, time_sec: u32) -> MpdResult<()>;
    fn repeat(&mut self, enabled: bool) -> MpdResult<()>;
    fn random(&mut self, enabled: bool) -> MpdResult<()>;
    fn single(&mut self, single: OnOffOneshot) -> MpdResult<()>;
    fn consume(&mut self, consume: OnOffOneshot) -> MpdResult<()>;
    // Current queue
    fn add(&mut self, path: &str) -> MpdResult<()>;
    fn clear(&mut self) -> MpdResult<()>;
    fn delete_id(&mut self, id: u32) -> MpdResult<()>;
    fn playlist_info(&mut self) -> MpdResult<Option<Vec<Song>>>;
    fn find(&mut self, filter: &[Filter<'_>]) -> MpdResult<Vec<Song>>;
    fn search(&mut self, filter: &[Filter<'_>]) -> MpdResult<Vec<Song>>;
    fn move_id(&mut self, id: u32, to: QueueMoveTarget) -> MpdResult<()>;
    fn find_one(&mut self, filter: &[Filter<'_>]) -> MpdResult<Option<Song>>;
    fn find_add(&mut self, filter: &[Filter<'_>]) -> MpdResult<()>;
    fn list_tag(&mut self, tag: Tag, filter: Option<&[Filter<'_>]>) -> MpdResult<MpdList>;
    // Database
    fn lsinfo(&mut self, path: Option<&str>) -> MpdResult<LsInfo>;
    fn list_files(&mut self, path: Option<&str>) -> MpdResult<ListFiles>;
    fn read_picture(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>>;
    fn albumart(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>>;
    // Stored playlists
    fn list_playlists(&mut self) -> MpdResult<Vec<Playlist>>;
    fn list_playlist(&mut self, name: &str) -> MpdResult<FileList>;
    fn list_playlist_info(&mut self, playlist: &str) -> MpdResult<Vec<Song>>;
    fn load_playlist(&mut self, name: &str) -> MpdResult<()>;
    fn rename_playlist(&mut self, name: &str, new_name: &str) -> MpdResult<()>;
    fn delete_playlist(&mut self, name: &str) -> MpdResult<()>;
    fn delete_from_playlist(&mut self, playlist_name: &str, songs: &SingleOrRange) -> MpdResult<()>;
    fn move_in_playlist(&mut self, playlist_name: &str, range: &SingleOrRange, target_position: usize)
        -> MpdResult<()>;
    fn add_to_playlist(&mut self, playlist_name: &str, uri: &str, target_position: Option<usize>) -> MpdResult<()>;
    fn save_queue_as_playlist(&mut self, name: &str, mode: Option<SaveMode>) -> MpdResult<()>;
    /// This function first invokes [`Self::albumart`].
    /// If no album art is fonud it invokes [`Self::read_picture`].
    /// If no art is still found, but no errors were encountered, None is returned.
    fn find_album_art(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>>;
}

impl MpdClient for Client<'_> {
    // Queries
    fn idle(&mut self) -> MpdResult<Vec<IdleEvent>> {
        self.send("idle").and_then(ProtoClient::read_response)
    }

    fn get_volume(&mut self) -> MpdResult<Volume> {
        self.send("getvol").and_then(ProtoClient::read_response)
    }

    fn set_volume(&mut self, volume: Volume) -> MpdResult<()> {
        self.send(&format!("setvol {}", volume.value()))
            .and_then(ProtoClient::read_ok)
    }

    fn get_current_song(&mut self) -> MpdResult<Option<Song>> {
        self.send("currentsong").and_then(ProtoClient::read_opt_response)
    }

    fn get_status(&mut self) -> MpdResult<Status> {
        self.send("status").and_then(ProtoClient::read_response)
    }

    // Playback control
    fn pause_toggle(&mut self) -> MpdResult<()> {
        self.send("pause").and_then(ProtoClient::read_ok)
    }

    fn next(&mut self) -> MpdResult<()> {
        self.send("next").and_then(ProtoClient::read_ok)
    }

    fn prev(&mut self) -> MpdResult<()> {
        self.send("previous").and_then(ProtoClient::read_ok)
    }

    fn play_pos(&mut self, pos: u32) -> MpdResult<()> {
        self.send(&format!("play {pos}")).and_then(ProtoClient::read_ok)
    }

    fn play(&mut self) -> MpdResult<()> {
        self.send("play").and_then(ProtoClient::read_ok)
    }

    fn play_id(&mut self, id: u32) -> MpdResult<()> {
        self.send(&format!("playid {id}")).and_then(ProtoClient::read_ok)
    }

    fn stop(&mut self) -> MpdResult<()> {
        self.send("stop").and_then(ProtoClient::read_ok)
    }

    fn seek_curr_forwards(&mut self, time_sec: u32) -> MpdResult<()> {
        self.send(&format!("seekcur +{time_sec}"))
            .and_then(ProtoClient::read_ok)
    }

    fn seek_curr_backwards(&mut self, time_sec: u32) -> MpdResult<()> {
        self.send(&format!("seekcur -{time_sec}"))
            .and_then(ProtoClient::read_ok)
    }

    fn repeat(&mut self, enabled: bool) -> MpdResult<()> {
        self.send(&format!("repeat {}", u8::from(enabled)))
            .and_then(ProtoClient::read_ok)
    }

    fn random(&mut self, enabled: bool) -> MpdResult<()> {
        self.send(&format!("random {}", u8::from(enabled)))
            .and_then(ProtoClient::read_ok)
    }

    fn single(&mut self, single: OnOffOneshot) -> MpdResult<()> {
        self.send(&format!("single {}", single.to_mpd_value()))
            .and_then(ProtoClient::read_ok)
    }

    fn consume(&mut self, consume: OnOffOneshot) -> MpdResult<()> {
        if self.version < Version::new(0, 24, 0) && matches!(consume, OnOffOneshot::Oneshot) {
            Err(MpdError::UnsupportedMpdVersion(
                "consume oneshot can be used since MPD 0.24.0",
            ))
        } else {
            self.send(&format!("consume {}", consume.to_mpd_value()))
                .and_then(ProtoClient::read_ok)
        }
    }

    // Current queue
    fn add(&mut self, path: &str) -> MpdResult<()> {
        self.send(&format!("add \"{path}\"")).and_then(ProtoClient::read_ok)
    }

    fn clear(&mut self) -> MpdResult<()> {
        self.send("clear").and_then(ProtoClient::read_ok)
    }

    fn delete_id(&mut self, id: u32) -> MpdResult<()> {
        self.send(&format!("deleteid \"{id}\"")).and_then(ProtoClient::read_ok)
    }

    fn move_id(&mut self, id: u32, to: QueueMoveTarget) -> MpdResult<()> {
        self.send(&format!("moveid \"{id}\" \"{}\"", to.as_mpd_str()))
            .and_then(ProtoClient::read_ok)
    }

    fn playlist_info(&mut self) -> MpdResult<Option<Vec<Song>>> {
        self.send("playlistinfo").and_then(ProtoClient::read_opt_response)
    }

    /// Search the database for songs matching FILTER
    fn find(&mut self, filter: &[Filter<'_>]) -> MpdResult<Vec<Song>> {
        self.send(&format!("find \"({})\"", filter.to_query_str()))
            .and_then(ProtoClient::read_response)
    }

    /// Search the database for songs matching FILTER (see Filters).
    /// Parameters have the same meaning as for find, except that search is not case sensitive.
    fn search(&mut self, filter: &[Filter<'_>]) -> MpdResult<Vec<Song>> {
        let query = filter.to_query_str();
        let query = query.as_str();
        log::debug!(query; "Searching for songs");
        self.send(&format!("search \"({query})\""))
            .and_then(ProtoClient::read_response)
    }

    fn find_one(&mut self, filter: &[Filter<'_>]) -> MpdResult<Option<Song>> {
        Ok(self
            .send(&format!("find \"({})\"", filter.to_query_str()))
            .and_then(ProtoClient::read_response::<Vec<Song>>)?
            .pop())
    }

    fn find_add(&mut self, filter: &[Filter<'_>]) -> MpdResult<()> {
        self.send(&format!("findadd \"({})\"", filter.to_query_str()))
            .and_then(ProtoClient::read_ok)
    }

    fn list_tag(&mut self, tag: Tag, filter: Option<&[Filter<'_>]>) -> MpdResult<MpdList> {
        self.send(&if let Some(filter) = filter {
            format!("list {tag} \"({})\"", filter.to_query_str())
        } else {
            format!("list {tag}")
        })
        .and_then(ProtoClient::read_response)
    }

    // Database
    fn lsinfo(&mut self, path: Option<&str>) -> MpdResult<LsInfo> {
        Ok(if let Some(path) = path {
            self.send(&format!("lsinfo \"{path}\""))
                .and_then(ProtoClient::read_opt_response)?
                .unwrap_or_default()
        } else {
            self.send("lsinfo")
                .and_then(ProtoClient::read_opt_response)?
                .unwrap_or_default()
        })
        //     Ok(self
    }

    fn list_files(&mut self, path: Option<&str>) -> MpdResult<ListFiles> {
        Ok(if let Some(path) = path {
            self.send(&format!("listfiles \"{path}\""))
                .and_then(ProtoClient::read_opt_response)?
                .unwrap_or_default()
        } else {
            self.send("listfiles")
                .and_then(ProtoClient::read_opt_response)?
                .unwrap_or_default()
        })
    }

    // Stored playlists
    fn list_playlists(&mut self) -> MpdResult<Vec<Playlist>> {
        self.send("listplaylists").and_then(ProtoClient::read_response)
    }
    fn list_playlist(&mut self, name: &str) -> MpdResult<FileList> {
        self.send(&format!("listplaylist \"{name}\""))
            .and_then(ProtoClient::read_response)
    }
    fn list_playlist_info(&mut self, playlist: &str) -> MpdResult<Vec<Song>> {
        self.send(&format!("listplaylistinfo \"{playlist}\""))
            .and_then(ProtoClient::read_response)
    }
    fn load_playlist(&mut self, name: &str) -> MpdResult<()> {
        self.send(&format!("load \"{name}\"")).and_then(ProtoClient::read_ok)
    }
    fn delete_playlist(&mut self, name: &str) -> MpdResult<()> {
        self.send(&format!("rm \"{name}\"")).and_then(ProtoClient::read_ok)
    }
    fn delete_from_playlist(&mut self, playlist_name: &str, range: &SingleOrRange) -> MpdResult<()> {
        self.send(&format!("playlistdelete \"{playlist_name}\" {}", range.as_mpd_range()))
            .and_then(ProtoClient::read_ok)
    }
    fn move_in_playlist(
        &mut self,
        playlist_name: &str,
        range: &SingleOrRange,
        target_position: usize,
    ) -> MpdResult<()> {
        self.send(&format!(
            "playlistmove \"{playlist_name}\" {} {target_position}",
            range.as_mpd_range()
        ))
        .and_then(ProtoClient::read_ok)
    }

    fn add_to_playlist(&mut self, playlist_name: &str, uri: &str, target_position: Option<usize>) -> MpdResult<()> {
        match target_position {
            Some(target_position) => self
                .send(&format!(r#"playlistadd "{playlist_name}" "{uri}" {target_position}"#))
                .and_then(ProtoClient::read_ok),
            None => self
                .send(&format!(r#"playlistadd "{playlist_name}" "{uri}""#))
                .and_then(ProtoClient::read_ok),
        }
    }

    fn rename_playlist(&mut self, name: &str, new_name: &str) -> MpdResult<()> {
        self.send(&format!("rename \"{name}\" \"{new_name}\""))
            .and_then(ProtoClient::read_ok)
    }

    fn save_queue_as_playlist(&mut self, name: &str, mode: Option<SaveMode>) -> MpdResult<()> {
        if let Some(mode) = mode {
            if self.version < Version::new(0, 24, 0) {
                return Err(MpdError::UnsupportedMpdVersion(
                    "save mode can be used since MPD 0.24.0",
                ));
            }
            self.send(&format!("save \"{name}\" \"{}\"", mode.as_ref()))
                .and_then(ProtoClient::read_ok)
        } else {
            self.send(&format!("save \"{name}\"")).and_then(ProtoClient::read_ok)
        }
    }

    fn read_picture(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>> {
        self.send(&format!("readpicture \"{path}\""))
            .and_then(ProtoClient::read_bin)
    }

    fn albumart(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>> {
        self.send(&format!("albumart \"{path}\""))
            .and_then(ProtoClient::read_bin)
    }

    fn find_album_art(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>> {
        match self.albumart(path) {
            Ok(Some(v)) => Ok(Some(v)),
            Ok(None)
            | Err(MpdError::Mpd(MpdFailureResponse {
                code: ErrorCode::NoExist,
                ..
            })) => match self.read_picture(path) {
                Ok(Some(p)) => Ok(Some(p)),
                Ok(None) => {
                    log::debug!("No album art found, falling back to placeholder image");
                    Ok(None)
                }
                Err(MpdError::Mpd(MpdFailureResponse {
                    code: ErrorCode::NoExist,
                    ..
                })) => {
                    log::debug!("No album art found, falling back to placeholder image");
                    Ok(None)
                }
                Err(e) => {
                    status_error!(error:? = e; "Failed to read picture");
                    Ok(None)
                }
            },
            Err(e) => {
                status_error!(error:? = e; "Failed to read picture",);
                Ok(None)
            }
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum QueueMoveTarget {
    RelativeAdd(usize),
    RelativeSub(usize),
    Absolute(usize),
}

impl QueueMoveTarget {
    fn as_mpd_str(&self) -> String {
        match self {
            QueueMoveTarget::RelativeAdd(v) => format!("+{v}"),
            QueueMoveTarget::RelativeSub(v) => format!("-{v}"),
            QueueMoveTarget::Absolute(v) => format!("{v}"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct SingleOrRange {
    pub start: usize,
    pub end: Option<usize>,
}

#[derive(Deref)]
pub struct Ranges(Vec<SingleOrRange>);

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

impl std::fmt::Display for Ranges {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.0.iter().peekable();
        while let Some(range) = iter.next() {
            if let Some(end) = range.end {
                write!(f, "[{}:{}]", range.start, end)?;
            } else {
                write!(f, "[{}]", range.start)?;
            }
            if iter.peek().is_some() {
                write!(f, ", ")?;
            }
        }
        Ok(())
    }
}

// TODO: rework
impl From<&BTreeSet<usize>> for Ranges {
    fn from(value: &BTreeSet<usize>) -> Self {
        let res = value
            .iter()
            .fold(vec![Vec::default()], |mut acc: Vec<Vec<usize>>, val| {
                let last = acc.last_mut().expect("There should always be at least one element. Using malformed range could potentionally result in loss of data se better to panic here.");
                if last.is_empty() || last.last().is_some_and(|v| v == &(val - 1)) {
                    last.push(*val);
                } else {
                    acc.push(vec![*val]);
                }
                acc
            });
        Ranges(
            res.iter()
                .filter_map(|range| {
                    if range.is_empty() {
                        None
                    } else if range.len() == 1 {
                        Some(SingleOrRange {
                            start: range[0],
                            end: None,
                        })
                    } else {
                        Some(SingleOrRange {
                            start: range[0],
                            end: Some(range[range.len() - 1] + 1),
                        })
                    }
                })
                .collect::<Vec<_>>(),
        )
    }
}

#[cfg(test)]
mod ranges_tests {
    use std::collections::BTreeSet;

    use super::{Ranges, SingleOrRange};

    #[test]
    fn simply_works() {
        let input = &BTreeSet::from([20, 1, 2, 3, 10, 16, 21, 22, 23, 24, 15, 25]);

        let result: Ranges = input.into();
        let result = result.0;

        assert_eq!(result[0], SingleOrRange { start: 1, end: Some(4) });
        assert_eq!(result[1], SingleOrRange { start: 10, end: None });
        assert_eq!(
            result[2],
            SingleOrRange {
                start: 15,
                end: Some(17)
            }
        );
        assert_eq!(
            result[3],
            SingleOrRange {
                start: 20,
                end: Some(26)
            }
        );
    }

    #[test]
    fn empty_set() {
        let input = &BTreeSet::new();

        let result: Ranges = input.into();
        let result = result.0;

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn single_value() {
        let input = &BTreeSet::from([5]);

        let result: Ranges = input.into();
        let result = result.0;

        assert_eq!(result[0], SingleOrRange { start: 5, end: None });
    }
}

trait StrExt {
    fn escape(self) -> String;
}
impl StrExt for &str {
    fn escape(self) -> String {
        self.replace('\\', r"\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)")
            .replace('\'', "\\\\'")
            .replace('\"', "\\\"")
    }
}

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq)]
#[strum(serialize_all = "PascalCase")]
pub enum Tag {
    Any,
    Artist,
    AlbumArtist,
    Album,
    Title,
    File,
    Genre,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FilterKind {
    Exact,
    StartsWith,
    Contains,
    Regex,
}

#[derive(Debug)]
pub struct Filter<'a> {
    pub tag: Tag,
    pub value: &'a str,
    pub kind: FilterKind,
}

#[allow(dead_code)]
impl<'a> Filter<'a> {
    pub fn new(tag: Tag, value: &'a str) -> Self {
        Self {
            tag,
            value,
            kind: FilterKind::Exact,
        }
    }

    pub fn new_with_kind(tag: Tag, value: &'a str, kind: FilterKind) -> Self {
        Self { tag, value, kind }
    }

    pub fn with_type(mut self, t: FilterKind) -> Self {
        self.kind = t;
        self
    }

    fn to_query_str(&self) -> String {
        match self.kind {
            FilterKind::Exact => format!("{} == '{}'", self.tag, self.value.escape()),
            FilterKind::StartsWith => format!("{} =~ '^{}'", self.tag, self.value.escape()),
            FilterKind::Contains => format!("{} =~ '.*{}.*'", self.tag, self.value.escape()),
            FilterKind::Regex => format!("{} =~ '{}'", self.tag, self.value.escape()),
        }
    }
}

trait FilterExt {
    fn to_query_str(&self) -> String;
}
impl FilterExt for &[Filter<'_>] {
    fn to_query_str(&self) -> String {
        self.iter().enumerate().fold(String::new(), |mut acc, (idx, filter)| {
            if idx > 0 {
                acc.push_str(&format!(" AND ({})", filter.to_query_str()));
            } else {
                acc.push_str(&format!("({})", filter.to_query_str()));
            }
            acc
        })
    }
}

#[cfg(test)]
mod strext_tests {
    use crate::mpd::mpd_client::StrExt;

    #[test]
    fn escapes_correctly() {
        let input: &'static str = r#"(Artist == "foo'bar")"#;

        assert_eq!(input.escape(), r#"\(Artist == \"foo\\'bar\"\)"#);
    }
}

#[cfg(test)]
mod filter_tests {
    use crate::mpd::mpd_client::{FilterExt, FilterKind, Tag};

    use super::Filter;

    #[test]
    fn single_value() {
        let input: &[Filter<'_>] = &[Filter::new(Tag::Artist, "mrs singer")];

        assert_eq!(input.to_query_str(), "(Artist == 'mrs singer')");
    }

    #[test]
    fn starts_with() {
        let input: &[Filter<'_>] = &[Filter::new_with_kind(Tag::Artist, "mrs singer", FilterKind::StartsWith)];

        assert_eq!(input.to_query_str(), "(Artist =~ '^mrs singer')");
    }

    #[test]
    fn exact() {
        let input: &[Filter<'_>] = &[Filter::new_with_kind(Tag::Album, "the greatest", FilterKind::Exact)];

        assert_eq!(input.to_query_str(), "(Album == 'the greatest')");
    }

    #[test]
    fn contains() {
        let input: &[Filter<'_>] = &[Filter::new_with_kind(Tag::Album, "the greatest", FilterKind::Contains)];

        assert_eq!(input.to_query_str(), "(Album =~ '.*the greatest.*')");
    }

    #[test]
    fn regex() {
        let input: &[Filter<'_>] = &[Filter::new_with_kind(
            Tag::Album,
            r"the greatest.*\s+[A-Za-z]+$",
            FilterKind::Regex,
        )];

        assert_eq!(input.to_query_str(), r"(Album =~ 'the greatest.*\\\\s+[A-Za-z]+$')");
    }

    #[test]
    fn multiple_values() {
        let input: &[Filter<'_>] = &[
            Filter::new(Tag::Album, "the greatest"),
            Filter::new(Tag::Artist, "mrs singer"),
        ];

        assert_eq!(
            input.to_query_str(),
            "(Album == 'the greatest') AND (Artist == 'mrs singer')"
        );
    }
}
