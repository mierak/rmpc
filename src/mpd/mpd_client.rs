use std::{
    borrow::Cow,
    fmt::Write as _,
    ops::{Range, RangeInclusive},
    str::FromStr,
};

use anyhow::Result;
use derive_more::Deref;
use deunicode::deunicode;
use itertools::Itertools;
use rand::seq::SliceRandom;
use strum::{AsRefStr, Display};

use super::{
    FromMpd, QueuePosition,
    client::Client,
    commands::{
        IdleEvent, ListFiles, LsInfo, Mounts, Playlist, Song, Status, Update, Volume,
        decoders::Decoders,
        list::MpdList,
        list_playlist::FileList,
        mpd_config::MpdConfig,
        outputs::Outputs,
        status::OnOffOneshot,
        stickers::{Sticker, Stickers, StickersWithFile},
        volume::Bound,
    },
    errors::{ErrorCode, MpdError, MpdFailureResponse},
    proto_client::{ProtoClient, SocketClient},
    version::Version,
};
use crate::shared::{ext::error::ErrorExt, macros::status_error};

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

pub enum ValueChange {
    Increase(u32),
    Decrease(u32),
    Set(u32),
}

impl FromStr for ValueChange {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            v if v.starts_with('-') => {
                Ok(ValueChange::Decrease(v.trim_start_matches('-').parse()?))
            }
            v if v.starts_with('+') => {
                Ok(ValueChange::Increase(v.trim_start_matches('+').parse()?))
            }
            v => Ok(ValueChange::Set(v.parse()?)),
        }
    }
}

impl ValueChange {
    fn to_mpd_str(&self) -> String {
        match self {
            ValueChange::Increase(val) => format!("+{val}"),
            ValueChange::Decrease(val) => format!("-{val}"),
            ValueChange::Set(val) => format!("{val}"),
        }
    }
}

#[allow(dead_code)]
pub trait MpdClient: Sized {
    fn version(&mut self) -> Version;
    fn config(&mut self) -> Option<&MpdConfig>;
    fn binary_limit(&mut self, limit: u64) -> MpdResult<()>;
    fn password(&mut self, password: &str) -> MpdResult<()>;
    fn commands(&mut self) -> MpdResult<MpdList>;
    fn update(&mut self, path: Option<&str>) -> MpdResult<Update>;
    fn rescan(&mut self, path: Option<&str>) -> MpdResult<Update>;
    fn idle(&mut self, subsystem: Option<IdleEvent>) -> MpdResult<Vec<IdleEvent>>;
    fn enter_idle(&mut self) -> MpdResult<ProtoClient<'static, '_, Self>>
    where
        Self: SocketClient;
    fn noidle(&mut self) -> MpdResult<()>;

    fn start_cmd_list(&mut self) -> Result<()>;
    fn start_cmd_list_ok(&mut self) -> Result<()>;
    fn execute_cmd_list(&mut self) -> MpdResult<ProtoClient<'static, '_, Self>>
    where
        Self: SocketClient;

    fn get_volume(&mut self) -> MpdResult<Volume>;
    fn set_volume(&mut self, volume: Volume) -> MpdResult<()>;
    /// Set playback volume relative to current
    fn volume(&mut self, change: ValueChange) -> MpdResult<()>;
    fn get_current_song(&mut self) -> MpdResult<Option<Song>>;
    fn get_status(&mut self) -> MpdResult<Status>;
    // Playback control
    fn pause_toggle(&mut self) -> MpdResult<()>;
    fn pause(&mut self) -> MpdResult<()>;
    fn unpause(&mut self) -> MpdResult<()>;
    fn next(&mut self) -> MpdResult<()>;
    fn prev(&mut self) -> MpdResult<()>;
    fn play_pos(&mut self, pos: usize) -> MpdResult<()>;
    fn play(&mut self) -> MpdResult<()>;
    fn play_id(&mut self, id: u32) -> MpdResult<()>;
    fn stop(&mut self) -> MpdResult<()>;
    fn seek_current(&mut self, value: ValueChange) -> MpdResult<()>;
    fn repeat(&mut self, enabled: bool) -> MpdResult<()>;
    fn random(&mut self, enabled: bool) -> MpdResult<()>;
    fn single(&mut self, single: OnOffOneshot) -> MpdResult<()>;
    fn consume(&mut self, consume: OnOffOneshot) -> MpdResult<()>;
    // Mounts
    fn mount(&mut self, name: &str, path: &str) -> MpdResult<()>;
    fn unmount(&mut self, name: &str) -> MpdResult<()>;
    fn list_mounts(&mut self) -> MpdResult<Mounts>;
    // Current queue
    fn add(&mut self, path: &str, position: Option<QueuePosition>) -> MpdResult<()>;
    fn clear(&mut self) -> MpdResult<()>;
    fn delete_id(&mut self, id: u32) -> MpdResult<()>;
    fn delete_from_queue(&mut self, songs: SingleOrRange) -> MpdResult<()>;
    fn playlist_info(&mut self, fetch_stickers: bool) -> MpdResult<Option<Vec<Song>>>;
    fn find(&mut self, filter: &[Filter<'_>]) -> MpdResult<Vec<Song>>;
    fn search(&mut self, filter: &[Filter<'_>]) -> MpdResult<Vec<Song>>;
    fn move_in_queue(&mut self, from: SingleOrRange, to: QueuePosition) -> MpdResult<()>;
    fn move_id(&mut self, id: u32, to: QueuePosition) -> MpdResult<()>;
    fn find_one(&mut self, filter: &[Filter<'_>]) -> MpdResult<Option<Song>>;
    fn find_add(&mut self, filter: &[Filter<'_>], position: Option<QueuePosition>)
    -> MpdResult<()>;
    fn search_add(
        &mut self,
        filter: &[Filter<'_>],
        position: Option<QueuePosition>,
    ) -> MpdResult<()>;
    fn list_tag(&mut self, tag: Tag, filter: Option<&[Filter<'_>]>) -> MpdResult<MpdList>;
    /// Shuffles the current queue.
    fn shuffle(&mut self, range: Option<SingleOrRange>) -> MpdResult<()>;
    // Database
    fn add_random_songs(&mut self, count: usize, filter: Option<&[Filter<'_>]>) -> MpdResult<()>;
    fn add_random_tag(&mut self, count: usize, tag: Tag) -> MpdResult<()>;
    /// Do not use this unless absolutely necessary
    fn list_all(&mut self, path: Option<&str>) -> MpdResult<LsInfo>;
    fn lsinfo(&mut self, path: Option<&str>) -> MpdResult<LsInfo>;
    fn list_files(&mut self, path: Option<&str>) -> MpdResult<ListFiles>;
    fn read_picture(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>>;
    fn albumart(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>>;
    // Stored playlists
    fn list_playlists(&mut self) -> MpdResult<Vec<Playlist>>;
    fn list_playlist(&mut self, name: &str) -> MpdResult<FileList>;
    fn list_playlist_info(
        &mut self,
        playlist: &str,
        range: Option<SingleOrRange>,
    ) -> MpdResult<Vec<Song>>;
    fn load_playlist(&mut self, name: &str, position: Option<QueuePosition>) -> MpdResult<()>;
    fn rename_playlist(&mut self, name: &str, new_name: &str) -> MpdResult<()>;
    fn delete_playlist(&mut self, name: &str) -> MpdResult<()>;
    fn delete_from_playlist(&mut self, playlist_name: &str, songs: &SingleOrRange)
    -> MpdResult<()>;
    fn move_in_playlist(
        &mut self,
        playlist_name: &str,
        range: &SingleOrRange,
        target_position: usize,
    ) -> MpdResult<()>;
    fn add_to_playlist(
        &mut self,
        playlist_name: &str,
        uri: &str,
        target_position: Option<usize>,
    ) -> MpdResult<()>;
    fn save_queue_as_playlist(&mut self, name: &str, mode: Option<SaveMode>) -> MpdResult<()>;
    /// This function first invokes [`Self::albumart`].
    /// If no album art is found it invokes [`Self::read_picture`].
    /// If no art is still found, but no errors were encountered, None is
    /// returned.
    fn find_album_art(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>>;
    // Outputs
    fn outputs(&mut self) -> MpdResult<Outputs>;
    fn toggle_output(&mut self, id: u32) -> MpdResult<()>;
    fn enable_output(&mut self, id: u32) -> MpdResult<()>;
    fn disable_output(&mut self, id: u32) -> MpdResult<()>;
    // Decoders
    fn decoders(&mut self) -> MpdResult<Decoders>;
    // Stickers

    /// Reads a sticker value for the specified object.
    fn sticker(&mut self, uri: &str, name: &str) -> MpdResult<Option<Sticker>>;
    /// Adds a sticker value to the specified object. If a sticker item with
    /// that name already exists, it is replaced.
    fn set_sticker(&mut self, uri: &str, name: &str, value: &str) -> MpdResult<()>;
    /// Deletes a sticker value from the specified object.
    fn delete_sticker(&mut self, uri: &str, name: &str) -> MpdResult<()>;
    /// Deletes all stickers from the specified object.
    fn delete_all_stickers(&mut self, uri: &str) -> MpdResult<()>;
    /// Lists the stickers for the specified object.
    fn list_stickers(&mut self, uri: &str) -> MpdResult<Stickers>;
    fn list_stickers_multiple(&mut self, uris: &[&str]) -> MpdResult<Vec<Stickers>>;
    // Searches the sticker database for stickers with the specified name, below
    // the specified directory (URI).
    fn find_stickers(&mut self, uri: &str, name: &str) -> MpdResult<StickersWithFile>;

    // Partitions
    fn switch_to_partition(&mut self, name: &str) -> MpdResult<()>;
    fn new_partition(&mut self, name: &str) -> MpdResult<()>;
    fn delete_partition(&mut self, name: &str) -> MpdResult<()>;
    fn list_partitions(&mut self) -> MpdResult<MpdList>;
    fn move_output(&mut self, output_name: &str) -> MpdResult<()>;
}

fn read_response<T: Default + FromMpd, S: SocketClient>(
    mut c: ProtoClient<'_, '_, S>,
) -> MpdResult<T> {
    c.read_response()
}

fn read_opt_response<T: Default + FromMpd, S: SocketClient>(
    mut c: ProtoClient<'_, '_, S>,
) -> MpdResult<Option<T>> {
    c.read_opt_response()
}

fn read_bin<S: SocketClient>(mut c: ProtoClient<'_, '_, S>) -> MpdResult<Option<Vec<u8>>> {
    c.read_bin()
}

fn read_ok<S: SocketClient>(mut c: ProtoClient<'_, '_, S>) -> MpdResult<()> {
    c.read_ok()
}

impl MpdClient for Client<'_> {
    fn version(&mut self) -> Version {
        self.version
    }

    fn config(&mut self) -> Option<&MpdConfig> {
        if self.config.is_none() {
            match self.send("config").and_then(read_response) {
                Ok(config) => {
                    self.config = Some(config);
                }
                Err(error) => {
                    log::debug!(error:?; "Cannot get MPD config, most likely not using socket connection");
                }
            }
        }

        self.config.as_ref()
    }

    fn binary_limit(&mut self, limit: u64) -> MpdResult<()> {
        self.send(&format!("binarylimit {limit}")).and_then(read_ok)
    }

    fn password(&mut self, password: &str) -> MpdResult<()> {
        self.send(&format!("password {}", password.quote_and_escape())).and_then(read_ok)
    }

    // Lists commands supported by the MPD server
    fn commands(&mut self) -> MpdResult<MpdList> {
        self.send("commands").and_then(read_response)
    }

    fn update(&mut self, path: Option<&str>) -> MpdResult<Update> {
        if let Some(path) = path {
            self.send(&format!("update {}", path.quote_and_escape())).and_then(read_response)
        } else {
            self.send("update").and_then(read_response)
        }
    }

    fn rescan(&mut self, path: Option<&str>) -> MpdResult<Update> {
        if let Some(path) = path {
            self.send(&format!("rescan {}", path.quote_and_escape())).and_then(read_response)
        } else {
            self.send("rescan").and_then(read_response)
        }
    }

    // Queries
    fn idle(&mut self, subsystem: Option<IdleEvent>) -> MpdResult<Vec<IdleEvent>> {
        if let Some(subsystem) = subsystem {
            self.send(&format!("idle {subsystem}")).and_then(read_response)
        } else {
            self.send("idle").and_then(read_response)
        }
    }

    fn enter_idle(&mut self) -> MpdResult<ProtoClient<'static, '_, Self>>
    where
        Self: SocketClient,
    {
        self.send("idle")
    }

    fn noidle(&mut self) -> MpdResult<()> {
        self.send("noidle").and_then(read_ok)
    }

    fn start_cmd_list(&mut self) -> Result<()> {
        self.send("command_list_begin")?;
        Ok(())
    }

    fn start_cmd_list_ok(&mut self) -> Result<()> {
        self.send("command_list_ok_begin")?;
        Ok(())
    }

    fn execute_cmd_list(&mut self) -> MpdResult<ProtoClient<'static, '_, Self>> {
        self.send("command_list_end")
    }

    fn get_volume(&mut self) -> MpdResult<Volume> {
        if self.version < Version::new(0, 23, 0) {
            Err(MpdError::UnsupportedMpdVersion("getvol can be used since MPD 0.23.0"))
        } else {
            self.send("getvol").and_then(read_response)
        }
    }

    fn set_volume(&mut self, volume: Volume) -> MpdResult<()> {
        self.send(&format!("setvol {}", volume.value())).and_then(read_ok)
    }

    fn volume(&mut self, change: ValueChange) -> MpdResult<()> {
        match change {
            ValueChange::Increase(_) | ValueChange::Decrease(_) => {
                self.send(&format!("volume {}", change.to_mpd_str())).and_then(read_ok)
            }
            ValueChange::Set(val) => self.send(&format!("setvol {val}")).and_then(read_ok),
        }
    }

    fn get_current_song(&mut self) -> MpdResult<Option<Song>> {
        self.send("currentsong").and_then(read_opt_response)
    }

    fn get_status(&mut self) -> MpdResult<Status> {
        self.send("status").and_then(read_response)
    }

    // Playback control
    fn pause_toggle(&mut self) -> MpdResult<()> {
        self.send("pause").and_then(read_ok)
    }

    fn pause(&mut self) -> MpdResult<()> {
        self.send("pause 1").and_then(read_ok)
    }

    fn unpause(&mut self) -> MpdResult<()> {
        self.send("pause 0").and_then(read_ok)
    }

    fn next(&mut self) -> MpdResult<()> {
        self.send("next").and_then(read_ok)
    }

    fn prev(&mut self) -> MpdResult<()> {
        self.send("previous").and_then(read_ok)
    }

    fn play_pos(&mut self, pos: usize) -> MpdResult<()> {
        self.send(&format!("play {pos}")).and_then(read_ok)
    }

    fn play(&mut self) -> MpdResult<()> {
        self.send("play").and_then(read_ok)
    }

    fn play_id(&mut self, id: u32) -> MpdResult<()> {
        self.send(&format!("playid {id}")).and_then(read_ok)
    }

    fn stop(&mut self) -> MpdResult<()> {
        self.send("stop").and_then(read_ok)
    }

    fn seek_current(&mut self, value: ValueChange) -> MpdResult<()> {
        self.send(&format!("seekcur {}", value.to_mpd_str())).and_then(read_ok)
    }

    fn repeat(&mut self, enabled: bool) -> MpdResult<()> {
        self.send(&format!("repeat {}", u8::from(enabled))).and_then(read_ok)
    }

    fn random(&mut self, enabled: bool) -> MpdResult<()> {
        self.send(&format!("random {}", u8::from(enabled))).and_then(read_ok)
    }

    fn single(&mut self, single: OnOffOneshot) -> MpdResult<()> {
        self.send(&format!("single {}", single.to_mpd_value())).and_then(read_ok)
    }

    fn consume(&mut self, consume: OnOffOneshot) -> MpdResult<()> {
        if self.version < Version::new(0, 24, 0) && matches!(consume, OnOffOneshot::Oneshot) {
            Err(MpdError::UnsupportedMpdVersion("consume oneshot can be used since MPD 0.24.0"))
        } else {
            self.send(&format!("consume {}", consume.to_mpd_value())).and_then(read_ok)
        }
    }

    // Mounts
    fn mount(&mut self, name: &str, path: &str) -> MpdResult<()> {
        self.send(&format!("mount {} {}", name.quote_and_escape(), path.quote_and_escape()))
            .and_then(read_ok)
    }

    fn unmount(&mut self, name: &str) -> MpdResult<()> {
        self.send(&format!("unmount {}", name.quote_and_escape())).and_then(read_ok)
    }

    fn list_mounts(&mut self) -> MpdResult<Mounts> {
        self.send("listmounts").and_then(read_response)
    }

    // Current queue
    fn add(&mut self, uri: &str, position: Option<QueuePosition>) -> MpdResult<()> {
        let position_arg: String =
            position.map_or(String::new(), |v| format!(" {}", v.as_mpd_str()));
        self.send(&format!("add {}{position_arg}", uri.quote_and_escape())).and_then(read_ok)
    }

    fn clear(&mut self) -> MpdResult<()> {
        self.send("clear").and_then(read_ok)
    }

    fn delete_id(&mut self, id: u32) -> MpdResult<()> {
        self.send(&format!("deleteid {id}")).and_then(read_ok)
    }

    fn delete_from_queue(&mut self, songs: SingleOrRange) -> MpdResult<()> {
        self.send(&format!("delete {}", songs.as_mpd_range())).and_then(read_ok)
    }

    fn playlist_info(&mut self, fetch_stickers: bool) -> MpdResult<Option<Vec<Song>>> {
        let songs: Option<Vec<Song>> = self.send("playlistinfo").and_then(read_opt_response)?;

        if !fetch_stickers {
            return Ok(songs);
        }

        let Some(mut songs) = songs else {
            return Ok(songs);
        };

        let mut stickers = match self
            .list_stickers_multiple(&songs.iter().map(|song| song.file.as_str()).collect_vec())
        {
            Ok(stickers) => stickers,
            Err(err) => {
                log::error!(err:?; "Failed to fetch stickers for playlist_info");
                return Ok(Some(songs));
            }
        };

        if songs.len() != stickers.len() {
            log::error!(songs_len = songs.len(), stickers_len = stickers.len(); "Received different number of sticker responses than requested songs");
            return Ok(Some(songs));
        }

        for (stickers, song) in stickers.iter_mut().zip(songs.iter_mut()) {
            song.stickers = Some(std::mem::take(&mut stickers.0));
        }

        Ok(Some(songs))
    }

    /// Search the database for songs matching FILTER
    fn find(&mut self, filter: &[Filter<'_>]) -> MpdResult<Vec<Song>> {
        self.send(&format!("find \"({})\"", filter.to_query_str())).and_then(read_response)
    }

    /// Search the database for songs matching FILTER (see Filters).
    /// Parameters have the same meaning as for find, except that search is not
    /// case sensitive.
    fn search(&mut self, filter: &[Filter<'_>]) -> MpdResult<Vec<Song>> {
        let query = filter.to_query_str();
        let query = query.as_str();
        log::debug!(query; "Searching for songs");
        self.send(&format!("search \"({query})\"")).and_then(read_response)
    }

    fn move_in_queue(&mut self, from: SingleOrRange, to: QueuePosition) -> MpdResult<()> {
        self.send(&format!("move {} {}", from.as_mpd_range(), to.as_mpd_str())).and_then(read_ok)
    }

    fn move_id(&mut self, id: u32, to: QueuePosition) -> MpdResult<()> {
        self.send(&format!("moveid {id} \"{}\"", to.as_mpd_str())).and_then(read_ok)
    }

    fn find_one(&mut self, filter: &[Filter<'_>]) -> MpdResult<Option<Song>> {
        let mut songs: Vec<Song> =
            self.send(&format!("find \"({})\"", filter.to_query_str())).and_then(read_response)?;

        Ok(songs.pop())
    }

    fn find_add(
        &mut self,
        filter: &[Filter<'_>],
        position: Option<QueuePosition>,
    ) -> MpdResult<()> {
        let position_arg: String =
            position.map_or(String::new(), |v| format!(" position {}", v.as_mpd_str()));
        self.send(&format!("findadd \"({})\"{position_arg}", filter.to_query_str()))
            .and_then(read_ok)
    }

    /// Search the database for songs matching FILTER (see Filters) AND add them
    /// to queue. Parameters have the same meaning as for find, except that
    /// search is not case sensitive.
    fn search_add(
        &mut self,
        filter: &[Filter<'_>],
        position: Option<QueuePosition>,
    ) -> MpdResult<()> {
        let query = filter.to_query_str();
        let query = query.as_str();
        let position_arg: String =
            position.map_or(String::new(), |v| format!(" position {}", v.as_mpd_str()));
        log::debug!(query; "Searching for songs and adding them");
        self.send(&format!("searchadd \"({query})\"{position_arg}")).and_then(read_ok)
    }

    fn list_tag(&mut self, tag: Tag, filter: Option<&[Filter<'_>]>) -> MpdResult<MpdList> {
        self.send(&if let Some(filter) = filter {
            format!("list {} \"({})\"", tag.as_str(), filter.to_query_str())
        } else {
            format!("list {}", tag.as_str())
        })
        .and_then(read_response)
    }

    fn shuffle(&mut self, range: Option<SingleOrRange>) -> MpdResult<()> {
        if let Some(range) = range {
            self.send(&format!("shuffle {}", range.as_mpd_range())).and_then(read_ok)
        } else {
            self.send("shuffle").and_then(read_ok)
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn add_random_songs(&mut self, count: usize, filter: Option<&[Filter<'_>]>) -> MpdResult<()> {
        let mut result = if let Some(filter) = filter {
            self.find(filter)?.into_iter().map(|song| song.file).collect_vec()
        } else {
            self.list_all(None)?.into_files().collect_vec()
        };

        if result.len() < count {
            return Err(MpdError::Generic(format!(
                "Cannot add {count} songs. The database contains only {} entries.",
                result.len()
            )));
        }
        result.shuffle(&mut rand::rng());

        self.start_cmd_list()?;
        for i in 0..count {
            self.send(&format!("add {}", result[i].quote_and_escape()))?;
        }
        self.execute_cmd_list().and_then(read_ok)
    }

    #[allow(clippy::needless_range_loop)]
    fn add_random_tag(&mut self, count: usize, tag: Tag) -> MpdResult<()> {
        let mut tag_values = self.list_tag(tag.clone(), None)?.0;

        if tag_values.len() < count {
            return Err(MpdError::Generic(format!(
                "Cannot add {count} {tag}s. The database contains only {} entries.",
                tag_values.len()
            )));
        }

        tag_values.shuffle(&mut rand::rng());

        self.start_cmd_list()?;
        for i in 0..count {
            let filter = &[Filter::new_with_kind(
                tag.clone(),
                std::mem::take(&mut tag_values[i]),
                FilterKind::Exact,
            )] as &[_];
            self.send(&format!("findadd \"({})\"", filter.to_query_str()))?;
        }
        self.execute_cmd_list().and_then(read_ok)
    }

    fn list_all(&mut self, path: Option<&str>) -> MpdResult<LsInfo> {
        if let Some(path) = path {
            self.send(&format!("listall {}", path.quote_and_escape())).and_then(read_response)
        } else {
            self.send("listall").and_then(read_response)
        }
    }

    // Database
    fn lsinfo(&mut self, path: Option<&str>) -> MpdResult<LsInfo> {
        Ok(if let Some(path) = path {
            self.send(&format!("lsinfo {}", path.quote_and_escape()))
                .and_then(read_opt_response)?
                .unwrap_or_default()
        } else {
            self.send("lsinfo").and_then(read_opt_response)?.unwrap_or_default()
        })
    }

    fn list_files(&mut self, path: Option<&str>) -> MpdResult<ListFiles> {
        Ok(if let Some(path) = path {
            self.send(&format!("listfiles {}", path.quote_and_escape()))
                .and_then(read_opt_response)?
                .unwrap_or_default()
        } else {
            self.send("listfiles").and_then(read_opt_response)?.unwrap_or_default()
        })
    }

    fn read_picture(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>> {
        self.send(&format!("readpicture {} 0", path.quote_and_escape())).and_then(read_bin)
    }

    fn albumart(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>> {
        self.send(&format!("albumart {} 0", path.quote_and_escape())).and_then(read_bin)
    }

    // Stored playlists
    fn list_playlists(&mut self) -> MpdResult<Vec<Playlist>> {
        self.send("listplaylists").and_then(read_response)
    }

    fn list_playlist(&mut self, name: &str) -> MpdResult<FileList> {
        self.send(&format!("listplaylist {}", name.quote_and_escape())).and_then(read_response)
    }

    fn list_playlist_info(
        &mut self,
        playlist: &str,
        range: Option<SingleOrRange>,
    ) -> MpdResult<Vec<Song>> {
        if let Some(range) = range {
            if self.version < Version::new(0, 24, 0) {
                return Err(MpdError::UnsupportedMpdVersion(
                    "listplaylistinfo with range can only be used since MPD 0.24.0",
                ));
            }
            self.send(&format!(
                "listplaylistinfo {} {}",
                playlist.quote_and_escape(),
                range.as_mpd_range()
            ))
            .and_then(read_response)
        } else {
            self.send(&format!("listplaylistinfo {}", playlist.quote_and_escape()))
                .and_then(read_response)
        }
    }

    fn load_playlist(&mut self, name: &str, position: Option<QueuePosition>) -> MpdResult<()> {
        let position_arg: String =
            position.map_or(String::new(), |v| format!(" {}", v.as_mpd_str()));
        self.send(&format!("load {} 0:{position_arg}", name.quote_and_escape())).and_then(read_ok)
    }

    fn rename_playlist(&mut self, name: &str, new_name: &str) -> MpdResult<()> {
        self.send(&format!("rename {} {}", name.quote_and_escape(), new_name.quote_and_escape()))
            .and_then(read_ok)
    }

    fn delete_playlist(&mut self, name: &str) -> MpdResult<()> {
        self.send(&format!("rm {}", name.quote_and_escape())).and_then(read_ok)
    }

    fn delete_from_playlist(
        &mut self,
        playlist_name: &str,
        range: &SingleOrRange,
    ) -> MpdResult<()> {
        self.send(&format!(
            "playlistdelete {} {}",
            playlist_name.quote_and_escape(),
            range.as_mpd_range()
        ))
        .and_then(read_ok)
    }

    fn move_in_playlist(
        &mut self,
        playlist_name: &str,
        range: &SingleOrRange,
        target_position: usize,
    ) -> MpdResult<()> {
        self.send(&format!(
            "playlistmove {} {} {target_position}",
            playlist_name.quote_and_escape(),
            range.as_mpd_range()
        ))
        .and_then(read_ok)
    }

    fn add_to_playlist(
        &mut self,
        playlist_name: &str,
        uri: &str,
        target_position: Option<usize>,
    ) -> MpdResult<()> {
        match target_position {
            Some(target_position) => self
                .send(&format!(
                    "playlistadd {} {} {target_position}",
                    playlist_name.quote_and_escape(),
                    uri.quote_and_escape()
                ))
                .and_then(read_ok),
            None => self
                .send(&format!(
                    "playlistadd {} {}",
                    playlist_name.quote_and_escape(),
                    uri.quote_and_escape()
                ))
                .and_then(read_ok),
        }
    }

    fn save_queue_as_playlist(&mut self, name: &str, mode: Option<SaveMode>) -> MpdResult<()> {
        if let Some(mode) = mode {
            if self.version < Version::new(0, 24, 0) {
                return Err(MpdError::UnsupportedMpdVersion(
                    "save mode can be used since MPD 0.24.0",
                ));
            }
            self.send(&format!("save {} \"{}\"", name.quote_and_escape(), mode.as_ref()))
                .and_then(read_ok)
        } else {
            self.send(&format!("save {}", name.quote_and_escape())).and_then(read_ok)
        }
    }

    fn find_album_art(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>> {
        // path is already escaped in albumart() and read_picture()
        match self.albumart(path) {
            Ok(Some(v)) => Ok(Some(v)),
            Ok(None) | Err(MpdError::Mpd(MpdFailureResponse { code: ErrorCode::NoExist, .. })) => {
                match self.read_picture(path) {
                    Ok(Some(p)) => Ok(Some(p)),
                    Ok(None) => {
                        log::debug!("No album art found, falling back to placeholder image");
                        Ok(None)
                    }
                    Err(MpdError::Mpd(MpdFailureResponse { code: ErrorCode::NoExist, .. })) => {
                        log::debug!("No album art found, falling back to placeholder image");
                        Ok(None)
                    }
                    Err(e) => {
                        status_error!(error:? = e; "Failed to read picture. {}", e.to_status());
                        Ok(None)
                    }
                }
            }
            Err(e) => {
                status_error!(error:? = e; "Failed to read picture. {}", e.to_status());
                Ok(None)
            }
        }
    }

    // Outputs
    fn outputs(&mut self) -> MpdResult<Outputs> {
        self.send("outputs").and_then(read_response)
    }

    fn toggle_output(&mut self, id: u32) -> MpdResult<()> {
        self.send(&format!("toggleoutput {id}")).and_then(read_ok)
    }

    fn enable_output(&mut self, id: u32) -> MpdResult<()> {
        self.send(&format!("enableoutput {id}")).and_then(read_ok)
    }

    fn disable_output(&mut self, id: u32) -> MpdResult<()> {
        self.send(&format!("disableoutput {id}")).and_then(read_ok)
    }

    // Decoders
    fn decoders(&mut self) -> MpdResult<Decoders> {
        self.send("decoders").and_then(read_response)
    }

    // Stickers
    fn sticker(&mut self, uri: &str, key: &str) -> MpdResult<Option<Sticker>> {
        let result: MpdResult<Sticker> = self
            .send(&format!(
                "sticker get song {} {}",
                uri.quote_and_escape(),
                key.quote_and_escape()
            ))
            .and_then(read_response);

        if let Err(MpdError::Mpd(MpdFailureResponse { code: ErrorCode::NoExist, .. })) = result {
            return Ok(None);
        }

        result.map(Some)
    }

    fn set_sticker(&mut self, uri: &str, key: &str, value: &str) -> MpdResult<()> {
        self.send(&format!(
            "sticker set song {} {} {}",
            uri.quote_and_escape(),
            key.quote_and_escape(),
            value.quote_and_escape()
        ))
        .and_then(read_ok)
    }

    fn delete_sticker(&mut self, uri: &str, key: &str) -> MpdResult<()> {
        self.send(&format!(
            "sticker delete song {} {}",
            uri.quote_and_escape(),
            key.quote_and_escape()
        ))
        .and_then(read_ok)
    }

    fn delete_all_stickers(&mut self, uri: &str) -> MpdResult<()> {
        self.send(&format!("sticker delete song {}", uri.quote_and_escape())).and_then(read_ok)
    }

    fn list_stickers(&mut self, uri: &str) -> MpdResult<Stickers> {
        self.send(&format!("sticker list song {}", uri.quote_and_escape())).and_then(read_response)
    }

    /// Resulting `Vec` is of the same length as input `uri`s.
    /// Default value (empty `HashMap`) is supplied if sticker
    /// for a specific URI cannot be found or an error is encountered
    fn list_stickers_multiple(&mut self, uris: &[&str]) -> MpdResult<Vec<Stickers>> {
        let mut result = Vec::with_capacity(uris.len());
        let mut list_ended_with_err = false;
        let mut i = 0;

        while i < uris.len() {
            self.start_cmd_list_ok()?;

            for uri in &uris[i..] {
                self.send(&format!("sticker list song {}", uri.quote_and_escape()))?;
            }
            let mut proto = self.execute_cmd_list()?;

            for uri in &uris[i..] {
                let res: MpdResult<Stickers> = proto.read_response();
                i += 1;
                match res {
                    Ok(v) => {
                        list_ended_with_err = false;
                        result.push(v);
                    }
                    Err(error) => {
                        log::warn!(error:?, uri; "Tried to find stickers but unexpected error occurred");
                        result.push(Stickers::default());
                        list_ended_with_err = true;
                        break;
                    }
                }
            }
        }

        // In case the last sticker was fetched successfully we have to read an
        // OK as an ack for the whole command list
        if !list_ended_with_err {
            ProtoClient::new_read_only(self).read_ok()?;
        }

        Ok(result)
    }

    fn find_stickers(&mut self, uri: &str, key: &str) -> MpdResult<StickersWithFile> {
        self.send(&format!(
            "sticker find song {} {}",
            uri.quote_and_escape(),
            key.quote_and_escape()
        ))
        .and_then(read_response)
    }

    fn switch_to_partition(&mut self, name: &str) -> MpdResult<()> {
        self.send(&format!("partition {}", name.quote_and_escape())).and_then(read_ok)
    }

    fn new_partition(&mut self, name: &str) -> MpdResult<()> {
        self.send(&format!("newpartition {}", name.quote_and_escape())).and_then(read_ok)
    }

    fn list_partitions(&mut self) -> MpdResult<MpdList> {
        self.send("listpartitions").and_then(read_response)
    }

    fn delete_partition(&mut self, name: &str) -> MpdResult<()> {
        self.send(&format!("delpartition {}", name.quote_and_escape())).and_then(read_ok)
    }

    fn move_output(&mut self, output_name: &str) -> MpdResult<()> {
        self.send(&format!("moveoutput {}", output_name.quote_and_escape())).and_then(read_ok)
    }
}

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

#[derive(Debug, PartialEq, Eq, Clone, Display)]
#[strum(serialize_all = "lowercase")]
#[allow(unused)]
pub enum Tag {
    Any,
    Artist,
    AlbumArtist,
    Album,
    Title,
    File,
    Genre,
    Custom(String),
}

impl Tag {
    fn as_str(&self) -> &str {
        match self {
            Tag::Any => "Any",
            Tag::Artist => "Artist",
            Tag::AlbumArtist => "AlbumArtist",
            Tag::Album => "Album",
            Tag::Title => "Title",
            Tag::File => "File",
            Tag::Genre => "Genre",
            Tag::Custom(v) => v,
        }
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FilterKind {
    Exact,
    StartsWith,
    #[default]
    Contains,
    Regex,
}

#[derive(Debug)]
pub struct Filter<'value> {
    pub tag: Tag,
    pub value: Cow<'value, str>,
    pub kind: FilterKind,
    pub ignore_diacritics: bool,
}

impl From<String> for Tag {
    fn from(value: String) -> Self {
        Self::Custom(value)
    }
}

#[allow(dead_code)]
impl<'value> Filter<'value> {
    pub fn new<T: Into<Tag>, V: Into<Cow<'value, str>>>(tag: T, value: V) -> Self {
        Self {
            tag: tag.into(),
            value: value.into(),
            kind: FilterKind::Exact,
            ignore_diacritics: false,
        }
    }

    pub fn new_with_kind<T: Into<Tag>, V: Into<Cow<'value, str>>>(
        tag: T,
        value: V,
        kind: FilterKind,
    ) -> Self {
        Self { tag: tag.into(), value: value.into(), kind, ignore_diacritics: false }
    }

    pub fn with_type(mut self, t: FilterKind) -> Self {
        self.kind = t;
        self
    }

    pub fn with_ignore_diacritics(mut self, ignore: bool) -> Self {
        self.ignore_diacritics = ignore;
        self
    }

    pub fn normalize_for_diacritics_unicode(&self) -> String {
        if !self.ignore_diacritics {
            return self.value.to_string();
        }

        deunicode(&self.value)
    }

    pub fn to_query_str(&self) -> String {
        if self.ignore_diacritics {
            let normalized_value = self.normalize_for_diacritics_unicode();

            match self.kind {
                FilterKind::Exact => {
                    let original_escaped = self.value.escape_filter();
                    let normalized_escaped = normalized_value.escape_filter();

                    if original_escaped == normalized_escaped {
                        format!("{} == '{}'", self.tag.as_str(), original_escaped)
                    } else {
                        let pattern = format!("(?i)^({normalized_escaped})$");
                        format!("{} =~ '{}'", self.tag.as_str(), pattern)
                    }
                }
                FilterKind::StartsWith => {
                    let pattern = format!("(?i)^{}", normalized_value.escape_filter());
                    format!("{} =~ '{}'", self.tag.as_str(), pattern)
                }
                FilterKind::Contains => {
                    let pattern = format!("(?i).*{}.*", normalized_value.escape_filter());
                    format!("{} =~ '{}'", self.tag.as_str(), pattern)
                }
                FilterKind::Regex => {
                    let pattern = if normalized_value.starts_with("(?i)") {
                        normalized_value.escape_filter()
                    } else {
                        format!("(?i){}", normalized_value.escape_filter())
                    };
                    format!("{} =~ '{}'", self.tag.as_str(), pattern)
                }
            }
        } else {
            match self.kind {
                FilterKind::Exact => {
                    format!("{} == '{}'", self.tag.as_str(), self.value.escape_filter())
                }
                FilterKind::StartsWith => {
                    format!("{} =~ '^{}'", self.tag.as_str(), self.value.escape_filter())
                }
                FilterKind::Contains => {
                    format!("{} =~ '.*{}.*'", self.tag.as_str(), self.value.escape_filter())
                }
                FilterKind::Regex => {
                    format!("{} =~ '{}'", self.tag.as_str(), self.value.escape_filter())
                }
            }
        }
    }
}

trait StrExt {
    fn escape_filter(self) -> String;
    fn quote_and_escape(self) -> String;
}

impl StrExt for &str {
    fn escape_filter(self) -> String {
        self.replace('\\', r"\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)")
            .replace('\'', "\\\\'")
            .replace('\"', "\\\"")
    }

    fn quote_and_escape(self) -> String {
        // reserve at least the input len + 2 for surrounding double quotes
        let mut result = String::with_capacity(self.len() + 2);

        result.push('"');
        for c in self.chars() {
            if c == '"' || c == '\\' {
                result.push('\\');
            }
            result.push(c);
        }

        result.push('"');
        result
    }
}

#[cfg(test)]
mod tests {
    use super::StrExt;

    #[test]
    fn strext_test() {
        let input = String::from("test\\test\",h,");

        let result = input.quote_and_escape();

        assert_eq!(result, "\"test\\\\test\\\",h,\"");
    }
}

trait FilterExt {
    fn to_query_str(&self) -> String;
}
impl FilterExt for &[Filter<'_>] {
    fn to_query_str(&self) -> String {
        self.iter().enumerate().fold(String::new(), |mut acc, (idx, filter)| {
            if idx > 0 {
                let _ = write!(acc, " AND ({})", filter.to_query_str());
            } else {
                let _ = write!(acc, "({})", filter.to_query_str());
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

        assert_eq!(input.escape_filter(), r#"\(Artist == \"foo\\'bar\"\)"#);
    }
}

#[cfg(test)]
mod filter_tests {
    use test_case::test_case;

    use super::Filter;
    use crate::mpd::mpd_client::{FilterExt, FilterKind, Tag};

    #[test_case(Tag::Artist, "Artist")]
    #[test_case(Tag::Album, "Album")]
    #[test_case(Tag::AlbumArtist, "AlbumArtist")]
    #[test_case(Tag::Title, "Title")]
    #[test_case(Tag::File, "File")]
    #[test_case(Tag::Genre, "Genre")]
    #[test_case(Tag::Custom("customtag".to_string()), "customtag")]
    fn single_value(tag: Tag, expected: &str) {
        let input: &[Filter<'_>] = &[Filter::new(tag, "mrs singer")];

        assert_eq!(input.to_query_str(), format!("({expected} == 'mrs singer')"));
    }

    #[test]
    fn starts_with() {
        let input: &[Filter<'_>] =
            &[Filter::new_with_kind(Tag::Artist, "mrs singer", FilterKind::StartsWith)];

        assert_eq!(input.to_query_str(), "(Artist =~ '^mrs singer')");
    }

    #[test]
    fn exact() {
        let input: &[Filter<'_>] =
            &[Filter::new_with_kind(Tag::Album, "the greatest", FilterKind::Exact)];

        assert_eq!(input.to_query_str(), "(Album == 'the greatest')");
    }

    #[test]
    fn contains() {
        let input: &[Filter<'_>] =
            &[Filter::new_with_kind(Tag::Album, "the greatest", FilterKind::Contains)];

        assert_eq!(input.to_query_str(), "(Album =~ '.*the greatest.*')");
    }

    #[test]
    fn regex() {
        let input: &[Filter<'_>] =
            &[Filter::new_with_kind(Tag::Album, r"the greatest.*\s+[A-Za-z]+$", FilterKind::Regex)];

        assert_eq!(input.to_query_str(), r"(Album =~ 'the greatest.*\\\\s+[A-Za-z]+$')");
    }

    #[test]
    fn multiple_values() {
        let input: &[Filter<'_>] =
            &[Filter::new(Tag::Album, "the greatest"), Filter::new(Tag::Artist, "mrs singer")];

        assert_eq!(input.to_query_str(), "(Album == 'the greatest') AND (Artist == 'mrs singer')");
    }

    #[cfg(test)]
    mod filter_diacritics_tests {
        use super::{Filter, FilterExt, FilterKind, Tag};
        use test_case::test_case;

        #[test]
        fn exact_with_ignore_diacritics() {
            let input: &[Filter<'_>] =
                &[Filter::new_with_kind(Tag::Artist, "Beyoncé", FilterKind::Exact)
                    .with_ignore_diacritics(true)];

            let query_str = input.to_query_str();

            assert!(query_str.contains("(?i)^(Beyonce)$"));
            assert!(query_str.contains("=~"));
            assert!(!query_str.contains("Beyoncé"));
        }

        #[test]
        fn starts_with_ignore_diacritics() {
            let input: &[Filter<'_>] =
                &[Filter::new_with_kind(Tag::Artist, "Café", FilterKind::StartsWith)
                    .with_ignore_diacritics(true)];

            let query_str = input.to_query_str();
            assert!(query_str.contains("(?i)^Cafe"));
            assert!(query_str.contains("=~"));
            assert!(!query_str.contains("Café"));
        }

        #[test]
        fn contains_with_ignore_diacritics() {
            let input: &[Filter<'_>] =
                &[Filter::new_with_kind(Tag::Title, "résumé", FilterKind::Contains)
                    .with_ignore_diacritics(true)];

            let query_str = input.to_query_str();
            assert!(query_str.contains("(?i).*resume.*"));
            assert!(query_str.contains("=~"));
            assert!(!query_str.contains("résumé"));
        }

        #[test]
        fn regex_with_ignore_diacritics() {
            let input: &[Filter<'_>] =
                &[Filter::new_with_kind(Tag::Title, "résumé.*test", FilterKind::Regex)
                    .with_ignore_diacritics(true)];

            let query_str = input.to_query_str();
            assert!(query_str.contains("(?i)resume.*test"));
            assert!(query_str.contains("=~"));
            assert!(!query_str.contains("résumé"));
        }

        #[test]
        fn regex_with_existing_case_insensitive_flag() {
            let input: &[Filter<'_>] =
                &[Filter::new_with_kind(Tag::Title, "résumé.*", FilterKind::Regex)
                    .with_ignore_diacritics(true)];

            let query_str = input.to_query_str();
            assert!(query_str.contains("resume.*"));
        }

        #[test_case("café", "cafe"; "french_e_acute")]
        #[test_case("naïve", "naive"; "french_i_diaeresis")]
        #[test_case("piñata", "pinata"; "spanish_n_tilde")]
        #[test_case("Zürich", "Zurich"; "german_u_umlaut")]
        #[test_case("François", "Francois"; "french_c_cedilla")]
        #[test_case("Bjørk", "Bjork"; "norwegian_o_slash")]
        #[test_case("Åsa", "Asa"; "swedish_a_ring")]
        fn various_diacritics_normalized(input: &str, expected: &str) {
            let filter = Filter::new_with_kind(Tag::Artist, input, FilterKind::Exact)
                .with_ignore_diacritics(true);
            let query_str = filter.to_query_str();

            if input == expected {
                assert!(query_str.contains(expected));
            } else {
                assert!(query_str.contains(expected));
                assert!(!query_str.contains(input));
            }
        }

        #[test]
        fn normalize_for_diacritics_unicode_works() {
            let filter = Filter::new(Tag::Artist, "Beyoncé").with_ignore_diacritics(true);
            assert_eq!(filter.normalize_for_diacritics_unicode(), "Beyonce");

            let filter_no_normalize =
                Filter::new(Tag::Artist, "Beyoncé").with_ignore_diacritics(false);
            assert_eq!(filter_no_normalize.normalize_for_diacritics_unicode(), "Beyoncé");
        }

        #[test]
        fn normalize_no_diacritics() {
            let filter = Filter::new(Tag::Artist, "Beatles").with_ignore_diacritics(true);
            assert_eq!(filter.normalize_for_diacritics_unicode(), "Beatles");
        }

        #[test]
        fn empty_string_with_diacritics_ignore() {
            let input: &[Filter<'_>] = &[Filter::new_with_kind(Tag::Artist, "", FilterKind::Exact)
                .with_ignore_diacritics(true)];

            let query_str = input.to_query_str();
            assert!(query_str.contains("== ''"));
        }

        #[test]
        fn only_diacritics_string() {
            let input: &[Filter<'_>] =
                &[Filter::new_with_kind(Tag::Artist, "éàü", FilterKind::Exact)
                    .with_ignore_diacritics(true)];

            let query_str = input.to_query_str();
            assert!(query_str.contains("eau"));
            assert!(!query_str.contains("éàü"));
        }

        #[test]
        fn diacritics_with_regex_special_chars() {
            let input: &[Filter<'_>] =
                &[Filter::new_with_kind(Tag::Title, "café[test]", FilterKind::Exact)
                    .with_ignore_diacritics(true)];

            let query_str = input.to_query_str();
            assert!(query_str.contains("cafe[test]"));
            assert!(!query_str.contains("café"));
        }

        #[test]
        fn diacritics_with_quotes_and_backslashes() {
            let input: &[Filter<'_>] =
                &[Filter::new_with_kind(Tag::Title, "café\"test\\", FilterKind::Contains)
                    .with_ignore_diacritics(true)];

            let query_str = input.to_query_str();
            assert!(query_str.contains("cafe\\\"test\\\\\\\\"));
            assert!(!query_str.contains("café"));
        }
    }
}
