use std::str::FromStr;

use anyhow::Result;
use strum::AsRefStr;

use super::{
    commands::{
        IdleEvent,
        ListFiles,
        LsInfo,
        Mounts,
        Playlist,
        Song,
        Status,
        Update,
        Volume,
        decoders::Decoders,
        list::MpdList,
        list_playlist::FileList,
        mpd_config::MpdConfig,
        outputs::Outputs,
        status::OnOffOneshot,
        stickers::{Sticker, Stickers, StickersWithFile},
        volume::Bound,
    },
    errors::MpdError,
    proto_client::{ProtoClient, SocketClient},
    version::Version,
};
use crate::{
    commands::list_all::ListAll,
    filter::{Filter, FilterExt, Tag},
    queue_position::QueuePosition,
    single_or_range::SingleOrRange,
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

pub enum ValueChange {
    Increase(u32),
    Decrease(u32),
    Set(u32),
}

#[derive(Debug, Default, Clone, Copy)]
pub enum AlbumArtOrder {
    #[default]
    EmbeddedFirst,
    FileFirst,
    EmbeddedOnly,
    FileOnly,
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
pub trait MpdCommand {
    fn send_binary_limit(&mut self, limit: u64) -> MpdResult<()>;
    fn send_password(&mut self, password: &str) -> MpdResult<()>;
    fn send_commands(&mut self) -> MpdResult<()>;
    fn send_not_commands(&mut self) -> MpdResult<()>;
    fn send_update(&mut self, path: Option<&str>) -> MpdResult<()>;
    fn send_rescan(&mut self, path: Option<&str>) -> MpdResult<()>;
    fn send_idle(&mut self, subsystem: Option<IdleEvent>) -> MpdResult<()>;
    fn send_noidle(&mut self) -> MpdResult<()>;
    fn send_start_cmd_list(&mut self) -> MpdResult<()>;
    fn send_start_cmd_list_ok(&mut self) -> MpdResult<()>;
    fn send_execute_cmd_list(&mut self) -> MpdResult<()>;
    fn send_get_volume(&mut self) -> MpdResult<()>;
    fn send_set_volume(&mut self, volume: Volume) -> MpdResult<()>;
    fn send_volume(&mut self, change: ValueChange) -> MpdResult<()>;
    fn send_crossfade(&mut self, seconds: u32) -> MpdResult<()>;
    fn send_get_current_song(&mut self) -> MpdResult<()>;
    fn send_get_status(&mut self) -> MpdResult<()>;
    fn send_pause_toggle(&mut self) -> MpdResult<()>;
    fn send_pause(&mut self) -> MpdResult<()>;
    fn send_unpause(&mut self) -> MpdResult<()>;
    fn send_next(&mut self) -> MpdResult<()>;
    fn send_prev(&mut self) -> MpdResult<()>;
    fn send_play_pos(&mut self, pos: usize) -> MpdResult<()>;
    fn send_play(&mut self) -> MpdResult<()>;
    fn send_play_id(&mut self, id: u32) -> MpdResult<()>;
    fn send_stop(&mut self) -> MpdResult<()>;
    fn send_seek_current(&mut self, value: ValueChange) -> MpdResult<()>;
    fn send_repeat(&mut self, enabled: bool) -> MpdResult<()>;
    fn send_random(&mut self, enabled: bool) -> MpdResult<()>;
    fn send_single(&mut self, single: OnOffOneshot) -> MpdResult<()>;
    fn send_consume(&mut self, consume: OnOffOneshot) -> MpdResult<()>;
    fn send_mount(&mut self, name: &str, path: &str) -> MpdResult<()>;
    fn send_unmount(&mut self, name: &str) -> MpdResult<()>;
    fn send_list_mounts(&mut self) -> MpdResult<()>;
    fn send_add(&mut self, path: &str, position: Option<QueuePosition>) -> MpdResult<()>;
    fn send_clear(&mut self) -> MpdResult<()>;
    fn send_swap_position(&mut self, song1: usize, song2: usize) -> MpdResult<()>;
    fn send_swap_id(&mut self, id1: u32, id2: u32) -> MpdResult<()>;
    fn send_delete_id(&mut self, id: u32) -> MpdResult<()>;
    fn send_delete_from_queue(&mut self, songs: SingleOrRange) -> MpdResult<()>;
    fn send_playlist_info(&mut self) -> MpdResult<()>;
    fn send_find(&mut self, filter: &[Filter<'_>]) -> MpdResult<()>;
    fn send_search(&mut self, filter: &[Filter<'_>]) -> MpdResult<()>;
    fn send_move_in_queue(&mut self, from: SingleOrRange, to: QueuePosition) -> MpdResult<()>;
    fn send_move_id(&mut self, id: u32, to: QueuePosition) -> MpdResult<()>;
    fn send_find_add(
        &mut self,
        filter: &[Filter<'_>],
        position: Option<QueuePosition>,
    ) -> MpdResult<()>;
    fn send_search_add(
        &mut self,
        filter: &[Filter<'_>],
        position: Option<QueuePosition>,
    ) -> MpdResult<()>;
    fn send_list_tag(&mut self, tag: Tag, filter: Option<&[Filter<'_>]>) -> MpdResult<()>;
    fn send_shuffle(&mut self, range: Option<SingleOrRange>) -> MpdResult<()>;
    fn send_list_all(&mut self, path: Option<&str>) -> MpdResult<()>;
    fn send_lsinfo(&mut self, path: Option<&str>) -> MpdResult<()>;
    fn send_list_files(&mut self, path: Option<&str>) -> MpdResult<()>;
    fn send_read_picture(&mut self, path: &str) -> MpdResult<String>;
    fn send_albumart(&mut self, path: &str) -> MpdResult<String>;
    fn send_list_playlists(&mut self) -> MpdResult<()>;
    fn send_list_playlist(&mut self, name: &str) -> MpdResult<()>;
    fn send_list_playlist_info(
        &mut self,
        playlist: &str,
        range: Option<SingleOrRange>,
    ) -> MpdResult<()>;
    fn send_load_playlist(&mut self, name: &str, position: Option<QueuePosition>) -> MpdResult<()>;
    fn send_rename_playlist(&mut self, name: &str, new_name: &str) -> MpdResult<()>;
    fn send_delete_playlist(&mut self, name: &str) -> MpdResult<()>;
    fn send_clear_playlist(&mut self, name: &str) -> MpdResult<()>;
    fn send_delete_from_playlist(
        &mut self,
        playlist_name: &str,
        songs: &SingleOrRange,
    ) -> MpdResult<()>;
    fn send_move_in_playlist(
        &mut self,
        playlist_name: &str,
        range: &SingleOrRange,
        target_position: usize,
    ) -> MpdResult<()>;
    fn send_add_to_playlist(
        &mut self,
        playlist_name: &str,
        uri: &str,
        target_position: Option<usize>,
    ) -> MpdResult<()>;
    fn send_save_queue_as_playlist(&mut self, name: &str, mode: Option<SaveMode>) -> MpdResult<()>;
    fn send_outputs(&mut self) -> MpdResult<()>;
    fn send_toggle_output(&mut self, id: u32) -> MpdResult<()>;
    fn send_enable_output(&mut self, id: u32) -> MpdResult<()>;
    fn send_disable_output(&mut self, id: u32) -> MpdResult<()>;
    fn send_decoders(&mut self) -> MpdResult<()>;
    fn send_sticker(&mut self, uri: &str, name: &str) -> MpdResult<()>;
    fn send_set_sticker(&mut self, uri: &str, name: &str, value: &str) -> MpdResult<()>;
    fn send_delete_sticker(&mut self, uri: &str, name: &str) -> MpdResult<()>;
    fn send_delete_all_stickers(&mut self, uri: &str) -> MpdResult<()>;
    fn send_list_stickers(&mut self, uri: &str) -> MpdResult<()>;
    fn send_find_stickers(
        &mut self,
        uri: &str,
        name: &str,
        filter: Option<StickerFilter>,
    ) -> MpdResult<()>;
    fn send_switch_to_partition(&mut self, name: &str) -> MpdResult<()>;
    fn send_new_partition(&mut self, name: &str) -> MpdResult<()>;
    fn send_delete_partition(&mut self, name: &str) -> MpdResult<()>;
    fn send_list_partitions(&mut self) -> MpdResult<()>;
    fn send_move_output(&mut self, output_name: &str) -> MpdResult<()>;
    fn send_send_message(&mut self, channel: &str, content: &str) -> MpdResult<()>;
    fn send_string_normalization_enable(
        &mut self,
        features: &[StringNormalizationFeature],
    ) -> MpdResult<()>;
    fn send_string_normalization_disable(
        &mut self,
        features: &[StringNormalizationFeature],
    ) -> MpdResult<()>;
    fn send_string_normalization_all(&mut self) -> MpdResult<()>;
    fn send_string_normalization_clear(&mut self) -> MpdResult<()>;
}

#[allow(dead_code)]
pub trait MpdClient: Sized {
    fn version(&mut self) -> Version;
    fn config(&mut self) -> Option<&MpdConfig>;
    fn binary_limit(&mut self, limit: u64) -> MpdResult<()>;
    fn password(&mut self, password: &str) -> MpdResult<()>;
    fn commands(&mut self) -> MpdResult<MpdList>;
    fn not_commands(&mut self) -> MpdResult<MpdList>;
    fn update(&mut self, path: Option<&str>) -> MpdResult<Update>;
    fn rescan(&mut self, path: Option<&str>) -> MpdResult<Update>;
    fn idle(&mut self, subsystem: Option<IdleEvent>) -> MpdResult<Vec<IdleEvent>>;
    fn enter_idle(&mut self, subsystem: Option<IdleEvent>) -> MpdResult<()>;
    fn noidle(&mut self) -> MpdResult<()>;

    fn get_volume(&mut self) -> MpdResult<Volume>;
    fn set_volume(&mut self, volume: Volume) -> MpdResult<()>;
    /// Set playback volume relative to current
    fn volume(&mut self, change: ValueChange) -> MpdResult<()>;
    fn crossfade(&mut self, seconds: u32) -> MpdResult<()>;
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
    // Swaps the songs at position SONG1 and SONG2 in the current playlist. Zero
    // based index.
    fn swap_position(&mut self, song1: usize, song2: usize) -> MpdResult<()>;
    fn swap_id(&mut self, id1: u32, id2: u32) -> MpdResult<()>;
    fn delete_id(&mut self, id: u32) -> MpdResult<()>;
    fn delete_from_queue(&mut self, songs: SingleOrRange) -> MpdResult<()>;
    fn playlist_info(&mut self) -> MpdResult<Option<Vec<Song>>>;
    fn find(&mut self, filter: &[Filter<'_>]) -> MpdResult<Vec<Song>>;
    fn search(&mut self, filter: &[Filter<'_>], ignore_diacritics: bool) -> MpdResult<Vec<Song>>;
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
    fn list_all(&mut self, path: Option<&str>) -> MpdResult<ListAll>;
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
    fn clear_playlist(&mut self, name: &str) -> MpdResult<()>;
    fn delete_from_playlist(&mut self, name: &str, songs: &SingleOrRange) -> MpdResult<()>;
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
    fn find_stickers(
        &mut self,
        uri: &str,
        name: &str,
        filter: Option<StickerFilter>,
    ) -> MpdResult<StickersWithFile>;

    // Partitions
    fn switch_to_partition(&mut self, name: &str) -> MpdResult<()>;
    fn new_partition(&mut self, name: &str) -> MpdResult<()>;
    fn delete_partition(&mut self, name: &str) -> MpdResult<()>;
    fn list_partitions(&mut self) -> MpdResult<MpdList>;
    fn move_output(&mut self, output_name: &str) -> MpdResult<()>;
    // Client to client
    fn send_message(&mut self, channel: &str, content: &str) -> MpdResult<()>;

    fn string_normalization_enable(
        &mut self,
        features: &[StringNormalizationFeature],
    ) -> MpdResult<()>;
    fn string_normalization_disable(
        &mut self,
        features: &[StringNormalizationFeature],
    ) -> MpdResult<()>;
    fn string_normalization_all(&mut self) -> MpdResult<()>;
    fn string_normalization_clear(&mut self) -> MpdResult<()>;
}

impl<T: SocketClient> MpdCommand for T {
    fn send_binary_limit(&mut self, limit: u64) -> MpdResult<()> {
        self.execute(&format!("binarylimit {limit}"))
    }

    fn send_password(&mut self, password: &str) -> MpdResult<()> {
        self.execute(&format!("password {}", password.quote_and_escape()))
    }

    fn send_commands(&mut self) -> MpdResult<()> {
        self.execute("commands")
    }

    fn send_not_commands(&mut self) -> MpdResult<()> {
        self.execute("notcommands")
    }

    fn send_update(&mut self, path: Option<&str>) -> MpdResult<()> {
        if let Some(path) = path {
            self.execute(&format!("update {}", path.quote_and_escape()))
        } else {
            self.execute("update")
        }
    }

    fn send_rescan(&mut self, path: Option<&str>) -> MpdResult<()> {
        if let Some(path) = path {
            self.execute(&format!("rescan {}", path.quote_and_escape()))
        } else {
            self.execute("rescan")
        }
    }

    fn send_idle(&mut self, subsystem: Option<IdleEvent>) -> MpdResult<()> {
        if let Some(subsystem) = subsystem {
            self.execute(&format!("idle {subsystem}"))
        } else {
            self.execute("idle")
        }
    }

    fn send_noidle(&mut self) -> MpdResult<()> {
        self.execute("noidle")
    }

    fn send_start_cmd_list(&mut self) -> MpdResult<()> {
        self.execute("command_list_begin")
    }

    fn send_start_cmd_list_ok(&mut self) -> MpdResult<()> {
        self.execute("command_list_ok_begin")
    }

    fn send_execute_cmd_list(&mut self) -> MpdResult<()> {
        self.execute("command_list_end")
    }

    fn send_get_volume(&mut self) -> MpdResult<()> {
        if self.version() < Version::new(0, 23, 0) {
            Err(MpdError::UnsupportedMpdVersion("getvol can be used since MPD 0.23.0"))
        } else {
            self.execute("getvol")
        }
    }

    fn send_set_volume(&mut self, volume: Volume) -> MpdResult<()> {
        self.execute(&format!("setvol {}", volume.value()))
    }

    fn send_volume(&mut self, change: ValueChange) -> MpdResult<()> {
        match change {
            ValueChange::Increase(_) | ValueChange::Decrease(_) => {
                self.execute(&format!("volume {}", change.to_mpd_str()))
            }
            ValueChange::Set(val) => self.execute(&format!("setvol {val}")),
        }
    }

    fn send_crossfade(&mut self, seconds: u32) -> MpdResult<()> {
        self.execute(&format!("crossfade {seconds}"))
    }

    fn send_get_current_song(&mut self) -> MpdResult<()> {
        self.execute("currentsong")
    }

    fn send_get_status(&mut self) -> MpdResult<()> {
        self.execute("status")
    }

    fn send_pause_toggle(&mut self) -> MpdResult<()> {
        self.execute("pause")
    }

    fn send_pause(&mut self) -> MpdResult<()> {
        self.execute("pause 1")
    }

    fn send_unpause(&mut self) -> MpdResult<()> {
        self.execute("pause 0")
    }

    fn send_next(&mut self) -> MpdResult<()> {
        self.execute("next")
    }

    fn send_prev(&mut self) -> MpdResult<()> {
        self.execute("previous")
    }

    fn send_play_pos(&mut self, pos: usize) -> MpdResult<()> {
        self.execute(&format!("play {pos}"))
    }

    fn send_play(&mut self) -> MpdResult<()> {
        self.execute("play")
    }

    fn send_play_id(&mut self, id: u32) -> MpdResult<()> {
        self.execute(&format!("playid {id}"))
    }

    fn send_stop(&mut self) -> MpdResult<()> {
        self.execute("stop")
    }

    fn send_seek_current(&mut self, value: ValueChange) -> MpdResult<()> {
        self.execute(&format!("seekcur {}", value.to_mpd_str()))
    }

    fn send_repeat(&mut self, enabled: bool) -> MpdResult<()> {
        self.execute(&format!("repeat {}", u8::from(enabled)))
    }

    fn send_random(&mut self, enabled: bool) -> MpdResult<()> {
        self.execute(&format!("random {}", u8::from(enabled)))
    }

    fn send_single(&mut self, single: OnOffOneshot) -> MpdResult<()> {
        self.execute(&format!("single {}", single.to_mpd_value()))
    }

    fn send_consume(&mut self, consume: OnOffOneshot) -> MpdResult<()> {
        if self.version() < Version::new(0, 24, 0) && matches!(consume, OnOffOneshot::Oneshot) {
            Err(MpdError::UnsupportedMpdVersion("consume oneshot can be used since MPD 0.24.0"))
        } else {
            self.execute(&format!("consume {}", consume.to_mpd_value()))
        }
    }

    fn send_mount(&mut self, name: &str, path: &str) -> MpdResult<()> {
        self.execute(&format!("mount {} {}", name.quote_and_escape(), path.quote_and_escape()))
    }

    fn send_unmount(&mut self, name: &str) -> MpdResult<()> {
        self.execute(&format!("unmount {}", name.quote_and_escape()))
    }

    fn send_list_mounts(&mut self) -> MpdResult<()> {
        self.execute("listmounts")
    }

    fn send_add(&mut self, uri: &str, position: Option<QueuePosition>) -> MpdResult<()> {
        let position_arg: String =
            position.map_or(String::new(), |v| format!(" {}", v.as_mpd_str()));
        self.execute(&format!("add {}{position_arg}", uri.quote_and_escape()))
    }

    fn send_clear(&mut self) -> MpdResult<()> {
        self.execute("clear")
    }

    fn send_swap_position(&mut self, song1: usize, song2: usize) -> MpdResult<()> {
        self.execute(&format!("swap {song1} {song2}"))
    }

    fn send_swap_id(&mut self, id1: u32, id2: u32) -> MpdResult<()> {
        self.execute(&format!("swapid {id1} {id2}"))
    }

    fn send_delete_id(&mut self, id: u32) -> MpdResult<()> {
        self.execute(&format!("deleteid {id}"))
    }

    fn send_delete_from_queue(&mut self, songs: SingleOrRange) -> MpdResult<()> {
        self.execute(&format!("delete {}", songs.as_mpd_range()))
    }

    fn send_playlist_info(&mut self) -> MpdResult<()> {
        self.execute("playlistinfo")
    }

    fn send_find(&mut self, filter: &[Filter<'_>]) -> MpdResult<()> {
        self.execute(&format!("find \"({})\"", filter.to_query_str()))
    }

    fn send_search(&mut self, filter: &[Filter<'_>]) -> MpdResult<()> {
        let query = filter.to_query_str();
        let query = query.as_str();
        log::debug!(query; "Searching for songs");
        self.execute(&format!("search \"({query})\""))
    }

    fn send_move_in_queue(&mut self, from: SingleOrRange, to: QueuePosition) -> MpdResult<()> {
        self.execute(&format!("move {} {}", from.as_mpd_range(), to.as_mpd_str()))
    }

    fn send_move_id(&mut self, id: u32, to: QueuePosition) -> MpdResult<()> {
        self.execute(&format!("moveid {id} \"{}\"", to.as_mpd_str()))
    }

    fn send_find_add(
        &mut self,
        filter: &[Filter<'_>],
        position: Option<QueuePosition>,
    ) -> MpdResult<()> {
        let position_arg: String =
            position.map_or(String::new(), |v| format!(" position {}", v.as_mpd_str()));
        self.execute(&format!("findadd \"({})\"{position_arg}", filter.to_query_str()))
    }

    fn send_search_add(
        &mut self,
        filter: &[Filter<'_>],
        position: Option<QueuePosition>,
    ) -> MpdResult<()> {
        let query = filter.to_query_str();
        let query = query.as_str();
        let position_arg: String =
            position.map_or(String::new(), |v| format!(" position {}", v.as_mpd_str()));
        log::debug!(query; "Searching for songs and adding them");
        self.execute(&format!("searchadd \"({query})\"{position_arg}"))
    }

    fn send_list_tag(&mut self, tag: Tag, filter: Option<&[Filter<'_>]>) -> MpdResult<()> {
        if let Some(filter) = filter {
            self.execute(&format!("list {} \"({})\"", tag.as_str(), filter.to_query_str()))
        } else {
            self.execute(&format!("list {}", tag.as_str()))
        }
    }

    fn send_shuffle(&mut self, range: Option<SingleOrRange>) -> MpdResult<()> {
        if let Some(range) = range {
            self.execute(&format!("shuffle {}", range.as_mpd_range()))
        } else {
            self.execute("shuffle")
        }
    }

    fn send_list_all(&mut self, path: Option<&str>) -> MpdResult<()> {
        if let Some(path) = path {
            self.execute(&format!("listall {}", path.quote_and_escape()))
        } else {
            self.execute("listall")
        }
    }

    fn send_lsinfo(&mut self, path: Option<&str>) -> MpdResult<()> {
        if let Some(path) = path {
            self.execute(&format!("lsinfo {}", path.quote_and_escape()))
        } else {
            self.execute("lsinfo")
        }
    }

    fn send_list_files(&mut self, path: Option<&str>) -> MpdResult<()> {
        if let Some(path) = path {
            self.execute(&format!("listfiles {}", path.quote_and_escape()))
        } else {
            self.execute("listfiles")
        }
    }

    fn send_read_picture(&mut self, path: &str) -> MpdResult<String> {
        let cmd = format!("readpicture {} 0", path.quote_and_escape());
        self.execute(&cmd)?;
        Ok(cmd)
    }

    fn send_albumart(&mut self, path: &str) -> MpdResult<String> {
        let cmd = format!("albumart {} 0", path.quote_and_escape());
        self.execute(&cmd)?;
        Ok(cmd)
    }

    fn send_list_playlists(&mut self) -> MpdResult<()> {
        self.execute("listplaylists")
    }

    fn send_list_playlist(&mut self, name: &str) -> MpdResult<()> {
        self.execute(&format!("listplaylist {}", name.quote_and_escape()))
    }

    fn send_list_playlist_info(
        &mut self,
        playlist: &str,
        range: Option<SingleOrRange>,
    ) -> MpdResult<()> {
        if let Some(range) = range {
            if self.version() < Version::new(0, 24, 0) {
                return Err(MpdError::UnsupportedMpdVersion(
                    "listplaylistinfo with range can only be used since MPD 0.24.0",
                ));
            }
            self.execute(&format!(
                "listplaylistinfo {} {}",
                playlist.quote_and_escape(),
                range.as_mpd_range()
            ))
        } else {
            self.execute(&format!("listplaylistinfo {}", playlist.quote_and_escape()))
        }
    }

    fn send_load_playlist(&mut self, name: &str, position: Option<QueuePosition>) -> MpdResult<()> {
        let position_arg: String =
            position.map_or(String::new(), |v| format!(" {}", v.as_mpd_str()));
        self.execute(&format!("load {} 0:{position_arg}", name.quote_and_escape()))
    }

    fn send_rename_playlist(&mut self, name: &str, new_name: &str) -> MpdResult<()> {
        self.execute(&format!("rename {} {}", name.quote_and_escape(), new_name.quote_and_escape()))
    }

    fn send_delete_playlist(&mut self, name: &str) -> MpdResult<()> {
        self.execute(&format!("rm {}", name.quote_and_escape()))
    }

    fn send_clear_playlist(&mut self, name: &str) -> MpdResult<()> {
        self.execute(&format!("playlistclear {}", name.quote_and_escape()))
    }

    fn send_delete_from_playlist(
        &mut self,
        playlist_name: &str,
        range: &SingleOrRange,
    ) -> MpdResult<()> {
        self.execute(&format!(
            "playlistdelete {} {}",
            playlist_name.quote_and_escape(),
            range.as_mpd_range()
        ))
    }

    fn send_move_in_playlist(
        &mut self,
        playlist_name: &str,
        range: &SingleOrRange,
        target_position: usize,
    ) -> MpdResult<()> {
        self.execute(&format!(
            "playlistmove {} {} {target_position}",
            playlist_name.quote_and_escape(),
            range.as_mpd_range()
        ))
    }

    fn send_add_to_playlist(
        &mut self,
        playlist_name: &str,
        uri: &str,
        target_position: Option<usize>,
    ) -> MpdResult<()> {
        match target_position {
            Some(target_position) => self.execute(&format!(
                "playlistadd {} {} {target_position}",
                playlist_name.quote_and_escape(),
                uri.quote_and_escape()
            )),
            None => self.execute(&format!(
                "playlistadd {} {}",
                playlist_name.quote_and_escape(),
                uri.quote_and_escape()
            )),
        }
    }

    fn send_save_queue_as_playlist(&mut self, name: &str, mode: Option<SaveMode>) -> MpdResult<()> {
        if let Some(mode) = mode {
            if self.version() < Version::new(0, 24, 0) {
                return Err(MpdError::UnsupportedMpdVersion(
                    "save mode can be used since MPD 0.24.0",
                ));
            }
            self.execute(&format!("save {} \"{}\"", name.quote_and_escape(), mode.as_ref()))
        } else {
            self.execute(&format!("save {}", name.quote_and_escape()))
        }
    }

    fn send_outputs(&mut self) -> MpdResult<()> {
        self.execute("outputs")
    }

    fn send_toggle_output(&mut self, id: u32) -> MpdResult<()> {
        self.execute(&format!("toggleoutput {id}"))
    }

    fn send_enable_output(&mut self, id: u32) -> MpdResult<()> {
        self.execute(&format!("enableoutput {id}"))
    }

    fn send_disable_output(&mut self, id: u32) -> MpdResult<()> {
        self.execute(&format!("disableoutput {id}"))
    }

    fn send_decoders(&mut self) -> MpdResult<()> {
        self.execute("decoders")
    }

    fn send_sticker(&mut self, uri: &str, key: &str) -> MpdResult<()> {
        self.execute(&format!(
            "sticker get song {} {}",
            uri.quote_and_escape(),
            key.quote_and_escape()
        ))
    }

    fn send_set_sticker(&mut self, uri: &str, key: &str, value: &str) -> MpdResult<()> {
        self.execute(&format!(
            "sticker set song {} {} {}",
            uri.quote_and_escape(),
            key.quote_and_escape(),
            value.quote_and_escape()
        ))
    }

    fn send_delete_sticker(&mut self, uri: &str, key: &str) -> MpdResult<()> {
        self.execute(&format!(
            "sticker delete song {} {}",
            uri.quote_and_escape(),
            key.quote_and_escape()
        ))
    }

    fn send_delete_all_stickers(&mut self, uri: &str) -> MpdResult<()> {
        self.execute(&format!("sticker delete song {}", uri.quote_and_escape()))
    }

    fn send_list_stickers(&mut self, uri: &str) -> MpdResult<()> {
        self.execute(&format!("sticker list song {}", uri.quote_and_escape()))
    }

    fn send_find_stickers(
        &mut self,
        uri: &str,
        key: &str,
        filter: Option<StickerFilter>,
    ) -> MpdResult<()> {
        if let Some(filter) = filter {
            self.execute(&format!(
                "sticker find song {} {} {}",
                uri.quote_and_escape(),
                key.quote_and_escape(),
                filter.as_mpd_str(),
            ))
        } else {
            self.execute(&format!(
                "sticker find song {} {}",
                uri.quote_and_escape(),
                key.quote_and_escape(),
            ))
        }
    }

    fn send_switch_to_partition(&mut self, name: &str) -> MpdResult<()> {
        self.execute(&format!("partition {}", name.quote_and_escape()))
    }

    fn send_new_partition(&mut self, name: &str) -> MpdResult<()> {
        self.execute(&format!("newpartition {}", name.quote_and_escape()))
    }

    fn send_delete_partition(&mut self, name: &str) -> MpdResult<()> {
        self.execute(&format!("delpartition {}", name.quote_and_escape()))
    }

    fn send_list_partitions(&mut self) -> MpdResult<()> {
        self.execute("listpartitions")
    }

    fn send_move_output(&mut self, output_name: &str) -> MpdResult<()> {
        self.execute(&format!("moveoutput {}", output_name.quote_and_escape()))
    }

    fn send_send_message(&mut self, channel: &str, content: &str) -> MpdResult<()> {
        self.execute(&format!(
            "sendmessage {} {}",
            channel.quote_and_escape(),
            content.quote_and_escape(),
        ))
    }

    fn send_string_normalization_enable(
        &mut self,
        features: &[StringNormalizationFeature],
    ) -> MpdResult<()> {
        debug_assert!(!features.is_empty());

        let mut buf = String::from("stringnormalization enable");
        for feature in features {
            buf.push(' ');
            buf.push_str(feature.as_ref());
        }
        self.execute(&buf)
    }

    fn send_string_normalization_disable(
        &mut self,
        features: &[StringNormalizationFeature],
    ) -> MpdResult<()> {
        debug_assert!(!features.is_empty());

        let mut buf = String::from("stringnormalization disable");
        for feature in features {
            buf.push(' ');
            buf.push_str(feature.as_ref());
        }
        self.execute(&buf)
    }

    fn send_string_normalization_all(&mut self) -> MpdResult<()> {
        self.execute("stringnormalization all")
    }

    fn send_string_normalization_clear(&mut self) -> MpdResult<()> {
        self.execute("stringnormalization clear")
    }
}

/// Sticker operators for filtering stickers in `find_stickers`
/// The *Int variants cast the sticker value to an integer before comparing.
#[derive(Debug, PartialEq, Clone, strum::IntoStaticStr, strum::AsRefStr)]
pub enum StickerFilter {
    Equals(String),
    GreaterThan(String),
    LessThan(String),
    Contains(String),
    StartsWith(String),
    EqualsInt(i32),
    GreaterThanInt(i32),
    LessThanInt(i32),
}

impl StickerFilter {
    fn as_mpd_str(&self) -> String {
        match self {
            StickerFilter::Equals(value) => format!("= {}", value.quote_and_escape()),
            StickerFilter::GreaterThan(value) => format!("> {}", value.quote_and_escape()),
            StickerFilter::LessThan(value) => format!("< {}", value.quote_and_escape()),
            StickerFilter::Contains(value) => format!("contains {}", value.quote_and_escape()),
            StickerFilter::StartsWith(value) => format!("starts_with {}", value.quote_and_escape()),
            StickerFilter::EqualsInt(value) => format!("eq {value}"),
            StickerFilter::GreaterThanInt(value) => format!("gt {value}"),
            StickerFilter::LessThanInt(value) => format!("lt {value}"),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy, strum::Display, strum::AsRefStr)]
#[strum(serialize_all = "snake_case")]
pub enum StringNormalizationFeature {
    StripDiacritics,
}

pub(crate) trait StrExt {
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
mod strext_tests {
    use super::*;

    #[test]
    fn escapes_correctly() {
        let input: &'static str = r#"(Artist == "foo'bar")"#;

        assert_eq!(input.escape_filter(), r#"\(Artist == \"foo\\'bar\"\)"#);
    }

    #[test]
    fn strext_test() {
        let input = String::from("test\\test\",h,");

        let result = input.quote_and_escape();

        assert_eq!(result, "\"test\\\\test\\\",h,\"");
    }
}
