#[cfg(target_os = "linux")]
use std::os::linux::net::SocketAddrExt;
#[cfg(target_os = "linux")]
use std::os::unix::net::SocketAddr;
use std::{
    collections::HashSet,
    io::{BufRead, BufReader, Write},
    net::{Shutdown, TcpStream},
    os::unix::net::UnixStream,
};

use anyhow::Result;
use itertools::Itertools;
use log::debug;
use rand::seq::SliceRandom;

use crate::{
    address::{MpdAddress, MpdPassword},
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
        list_all::ListAll,
        list_playlist::FileList,
        mpd_config::MpdConfig,
        outputs::Outputs,
        status::OnOffOneshot,
        stickers::{Sticker, Stickers, StickersWithFile},
    },
    errors::{ErrorCode, MpdError, MpdFailureResponse},
    filter::{Filter, FilterKind, Tag},
    mpd_client::{
        MpdClient,
        MpdCommand,
        SaveMode,
        StickerFilter,
        StringNormalizationFeature,
        ValueChange,
    },
    proto_client::{ProtoClient, SocketClient},
    queue_position::QueuePosition,
    single_or_range::SingleOrRange,
    version::Version,
};

type MpdResult<T> = Result<T, MpdError>;

pub const MIN_SUPPORTED_VERSION: Version = Version { major: 0, minor: 23, patch: 5 };

pub struct Client<'name> {
    name: &'name str,
    rx: BufReader<TcpOrUnixStream>,
    pub stream: TcpOrUnixStream,
    addr: MpdAddress,
    password: Option<MpdPassword>,
    pub version: Version,
    pub config: Option<MpdConfig>,
    pub supported_commands: HashSet<String>,
    partition: Option<String>,
    autocreate_partition: bool,
}

impl std::fmt::Debug for Client<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client {{ name: {:?}, addr: {:?} }}", self.name, self.addr)
    }
}

pub enum TcpOrUnixStream {
    Unix(UnixStream),
    Tcp(TcpStream),
}

impl TcpOrUnixStream {
    fn set_write_timeout(&mut self, duration: Option<std::time::Duration>) -> std::io::Result<()> {
        match self {
            TcpOrUnixStream::Unix(s) => {
                s.set_write_timeout(duration)?;
            }
            TcpOrUnixStream::Tcp(s) => {
                s.set_write_timeout(duration)?;
            }
        }
        Ok(())
    }

    fn set_read_timeout(&mut self, duration: Option<std::time::Duration>) -> std::io::Result<()> {
        match self {
            TcpOrUnixStream::Unix(s) => {
                s.set_read_timeout(duration)?;
            }
            TcpOrUnixStream::Tcp(s) => {
                s.set_read_timeout(duration)?;
            }
        }
        Ok(())
    }

    pub fn try_clone(&self) -> std::io::Result<Self> {
        Ok(match self {
            TcpOrUnixStream::Unix(s) => TcpOrUnixStream::Unix(s.try_clone()?),
            TcpOrUnixStream::Tcp(s) => TcpOrUnixStream::Tcp(s.try_clone()?),
        })
    }

    pub fn shutdown_both(&mut self) -> std::io::Result<()> {
        match self {
            TcpOrUnixStream::Unix(s) => s.shutdown(Shutdown::Both),
            TcpOrUnixStream::Tcp(s) => s.shutdown(Shutdown::Both),
        }
    }
}

impl std::io::Read for TcpOrUnixStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            TcpOrUnixStream::Unix(s) => s.read(buf),
            TcpOrUnixStream::Tcp(s) => s.read(buf),
        }
    }
}

impl std::io::Write for TcpOrUnixStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            TcpOrUnixStream::Unix(s) => s.write(buf),
            TcpOrUnixStream::Tcp(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            TcpOrUnixStream::Unix(s) => s.flush(),
            TcpOrUnixStream::Tcp(s) => s.flush(),
        }
    }
}

#[allow(dead_code)]
impl<'name> Client<'name> {
    pub fn init(
        addr: MpdAddress,
        password: Option<MpdPassword>,
        name: &'name str,
        partition: Option<String>,
        autocreate_partition: bool,
    ) -> MpdResult<Client<'name>> {
        let mut stream = match addr {
            MpdAddress::IpAndPort(ref addr) => TcpOrUnixStream::Tcp(TcpStream::connect(addr)?),
            MpdAddress::SocketPath(ref addr) => TcpOrUnixStream::Unix(UnixStream::connect(addr)?),
            #[cfg(target_os = "linux")]
            MpdAddress::AbstractSocket(ref addr) => {
                let addr = SocketAddr::from_abstract_name(addr)?;
                TcpOrUnixStream::Unix(UnixStream::connect_addr(&addr)?)
            }
            #[cfg(not(target_os = "linux"))]
            MpdAddress::AbstractSocket(ref _addr) => {
                return Err(MpdError::Generic(
                    "Abstract socket only supported on Linux".to_string(),
                ));
            }
        };
        stream.set_write_timeout(None)?;
        stream.set_read_timeout(None)?;
        let mut rx = BufReader::new(stream.try_clone()?);

        let mut buf = String::new();
        rx.read_line(&mut buf)?;
        if !buf.starts_with("OK") {
            return Err(MpdError::Generic(format!("Handshake validation failed. '{buf}'")));
        }
        let Some(version): Option<Version> =
            buf.strip_prefix("OK MPD ").and_then(|v| v.parse().ok())
        else {
            return Err(MpdError::Generic(format!(
                "Handshake validation failed. Cannot parse version from '{buf}'"
            )));
        };

        debug!(name, addr:?, version = version.to_string().as_str(), handshake = buf.trim(); "MPD client initialized");

        let mut client = Self {
            name,
            rx,
            stream,
            addr,
            password,
            version,
            partition,
            autocreate_partition,
            config: None,
            supported_commands: HashSet::new(),
        };

        if let Some(MpdPassword(ref password)) = client.password.clone() {
            debug!("Used password auth to MPD");
            client.password(password)?;
        }

        if let Some(partition) = client.partition.clone() {
            debug!(partition = partition.as_str(); "Using partition");
            match client.switch_to_partition(&partition) {
                Ok(()) => {}
                Err(MpdError::Mpd(MpdFailureResponse { code: ErrorCode::NoExist, .. }))
                    if autocreate_partition =>
                {
                    client.new_partition(&partition)?;
                    client.switch_to_partition(&partition)?;
                }
                err @ Err(_) => err?,
            }
        }

        // 2^18 seems to be max limit supported by MPD and higher values dont
        // have any effect
        client.binary_limit(2u64.pow(18))?;
        client.supported_commands = client.commands()?.0.into_iter().collect();

        Ok(client)
    }

    pub fn reconnect(&mut self) -> MpdResult<&Client<'_>> {
        debug!(name = self.name, addr:? = self.addr; "trying to reconnect");
        let mut stream = match &self.addr {
            MpdAddress::IpAndPort(addr) => TcpOrUnixStream::Tcp(TcpStream::connect(addr)?),
            MpdAddress::SocketPath(addr) => TcpOrUnixStream::Unix(UnixStream::connect(addr)?),
            #[cfg(target_os = "linux")]
            MpdAddress::AbstractSocket(addr) => {
                let addr = SocketAddr::from_abstract_name(addr)?;
                TcpOrUnixStream::Unix(UnixStream::connect_addr(&addr)?)
            }
            #[cfg(not(target_os = "linux"))]
            MpdAddress::AbstractSocket(addr) => {
                return Err(MpdError::Generic(
                    "Abstract socket only supported on Linux".to_string(),
                ));
            }
        };
        stream.set_write_timeout(None)?;
        stream.set_read_timeout(None)?;
        let mut rx = BufReader::new(stream.try_clone()?);

        let mut buf = String::new();
        rx.read_line(&mut buf)?;
        if !buf.starts_with("OK") {
            return Err(MpdError::Generic(format!("Handshake validation failed. '{buf}'")));
        }

        let Some(version): Option<Version> =
            buf.strip_prefix("OK MPD ").and_then(|v| v.parse().ok())
        else {
            return Err(MpdError::Generic(format!(
                "Handshake validation failed. Cannot parse version from '{buf}'"
            )));
        };

        self.rx = rx;
        self.stream = stream;
        self.version = version;
        self.config = None;

        debug!(name = self.name, addr:? = self.addr, handshake = buf.trim(), version = version.to_string().as_str(); "MPD client initialized");

        if let Some(MpdPassword(password)) = &self.password.clone() {
            debug!("Used password auth to MPD");
            self.password(password)?;
        }

        if let Some(partition) = self.partition.clone() {
            debug!(partition = partition.as_str(); "Using partition");
            match self.switch_to_partition(&partition) {
                Ok(()) => {}
                Err(MpdError::Mpd(MpdFailureResponse { code: ErrorCode::NoExist, .. }))
                    if self.autocreate_partition =>
                {
                    self.new_partition(&partition)?;
                    self.switch_to_partition(&partition)?;
                }
                err @ Err(_) => err?,
            }
        }

        self.supported_commands = self.commands()?.0.into_iter().collect();

        self.binary_limit(1024 * 1024 * 5)?;

        Ok(self)
    }

    pub fn set_read_timeout(
        &mut self,
        timeout: Option<std::time::Duration>,
    ) -> std::io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }

    pub fn set_write_timeout(
        &mut self,
        timeout: Option<std::time::Duration>,
    ) -> std::io::Result<()> {
        self.stream.set_write_timeout(timeout)
    }

    fn clear_read_buf(&mut self) -> Result<()> {
        log::trace!("Reinitialized read buffer");
        self.rx = BufReader::new(self.stream.try_clone()?);
        Ok(())
    }
}

impl SocketClient for Client<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        Write::write_all(&mut self.stream, bytes)
    }

    fn read(&mut self) -> &mut impl BufRead {
        &mut self.rx
    }

    fn clear_read_buf(&mut self) -> Result<()> {
        self.clear_read_buf()
    }

    fn version(&self) -> Version {
        self.version
    }
}

impl MpdClient for Client<'_> {
    fn version(&mut self) -> Version {
        self.version
    }

    fn config(&mut self) -> Option<&MpdConfig> {
        if self.config.is_none() {
            match self.execute("config").and_then(|()| self.read_response()) {
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
        self.send_binary_limit(limit).and_then(|()| self.read_ok())
    }

    fn password(&mut self, password: &str) -> MpdResult<()> {
        self.send_password(password).and_then(|()| self.read_ok())
    }

    // Lists commands supported by the MPD server
    fn commands(&mut self) -> MpdResult<MpdList> {
        self.send_commands().and_then(|()| self.read_response())
    }

    fn not_commands(&mut self) -> MpdResult<MpdList> {
        self.send_not_commands().and_then(|()| self.read_response())
    }

    fn update(&mut self, path: Option<&str>) -> MpdResult<Update> {
        self.send_update(path).and_then(|()| self.read_response())
    }

    fn rescan(&mut self, path: Option<&str>) -> MpdResult<Update> {
        self.send_rescan(path).and_then(|()| self.read_response())
    }

    // Queries
    fn idle(&mut self, subsystem: Option<IdleEvent>) -> MpdResult<Vec<IdleEvent>> {
        self.send_idle(subsystem).and_then(|()| self.read_response())
    }

    fn enter_idle(&mut self, subsystem: Option<IdleEvent>) -> MpdResult<()> {
        self.send_idle(subsystem)
    }

    fn noidle(&mut self) -> MpdResult<()> {
        self.send_noidle().and_then(|()| self.read_ok())
    }

    fn get_volume(&mut self) -> MpdResult<Volume> {
        self.send_get_volume().and_then(|()| self.read_response())
    }

    fn set_volume(&mut self, volume: Volume) -> MpdResult<()> {
        self.send_set_volume(volume).and_then(|()| self.read_ok())
    }

    fn volume(&mut self, change: ValueChange) -> MpdResult<()> {
        self.send_volume(change).and_then(|()| self.read_ok())
    }

    fn crossfade(&mut self, seconds: u32) -> MpdResult<()> {
        self.send_crossfade(seconds).and_then(|()| self.read_ok())
    }

    fn get_current_song(&mut self) -> MpdResult<Option<Song>> {
        self.send_get_current_song().and_then(|()| self.read_opt_response())
    }

    fn get_status(&mut self) -> MpdResult<Status> {
        self.send_get_status().and_then(|()| self.read_response())
    }

    // Playback control
    fn pause_toggle(&mut self) -> MpdResult<()> {
        self.send_pause_toggle().and_then(|()| self.read_ok())
    }

    fn pause(&mut self) -> MpdResult<()> {
        self.send_pause().and_then(|()| self.read_ok())
    }

    fn unpause(&mut self) -> MpdResult<()> {
        self.send_unpause().and_then(|()| self.read_ok())
    }

    fn next(&mut self) -> MpdResult<()> {
        self.send_next().and_then(|()| self.read_ok())
    }

    fn prev(&mut self) -> MpdResult<()> {
        self.send_prev().and_then(|()| self.read_ok())
    }

    fn play_pos(&mut self, pos: usize) -> MpdResult<()> {
        self.send_play_pos(pos).and_then(|()| self.read_ok())
    }

    fn play(&mut self) -> MpdResult<()> {
        self.send_play().and_then(|()| self.read_ok())
    }

    fn play_id(&mut self, id: u32) -> MpdResult<()> {
        self.send_play_id(id).and_then(|()| self.read_ok())
    }

    fn stop(&mut self) -> MpdResult<()> {
        self.send_stop().and_then(|()| self.read_ok())
    }

    fn seek_current(&mut self, value: ValueChange) -> MpdResult<()> {
        self.send_seek_current(value).and_then(|()| self.read_ok())
    }

    fn repeat(&mut self, enabled: bool) -> MpdResult<()> {
        self.send_repeat(enabled).and_then(|()| self.read_ok())
    }

    fn random(&mut self, enabled: bool) -> MpdResult<()> {
        self.send_random(enabled).and_then(|()| self.read_ok())
    }

    fn single(&mut self, single: OnOffOneshot) -> MpdResult<()> {
        self.send_single(single).and_then(|()| self.read_ok())
    }

    fn consume(&mut self, consume: OnOffOneshot) -> MpdResult<()> {
        self.send_consume(consume).and_then(|()| self.read_ok())
    }

    // Mounts
    fn mount(&mut self, name: &str, path: &str) -> MpdResult<()> {
        self.send_mount(name, path).and_then(|()| self.read_ok())
    }

    fn unmount(&mut self, name: &str) -> MpdResult<()> {
        self.send_unmount(name).and_then(|()| self.read_ok())
    }

    fn list_mounts(&mut self) -> MpdResult<Mounts> {
        self.send_list_mounts().and_then(|()| self.read_response())
    }

    // Current queue
    fn add(&mut self, uri: &str, position: Option<QueuePosition>) -> MpdResult<()> {
        self.send_add(uri, position).and_then(|()| self.read_ok())
    }

    fn clear(&mut self) -> MpdResult<()> {
        self.send_clear().and_then(|()| self.read_ok())
    }

    fn swap_position(&mut self, song1: usize, song2: usize) -> MpdResult<()> {
        self.send_swap_position(song1, song2).and_then(|()| self.read_ok())
    }

    fn swap_id(&mut self, id1: u32, id2: u32) -> MpdResult<()> {
        self.send_swap_id(id1, id2).and_then(|()| self.read_ok())
    }

    fn delete_id(&mut self, id: u32) -> MpdResult<()> {
        self.send_delete_id(id).and_then(|()| self.read_ok())
    }

    fn delete_from_queue(&mut self, songs: SingleOrRange) -> MpdResult<()> {
        self.send_delete_from_queue(songs).and_then(|()| self.read_ok())
    }

    fn playlist_info(&mut self) -> MpdResult<Option<Vec<Song>>> {
        self.send_playlist_info().and_then(|()| self.read_opt_response())
    }

    /// Search the database for songs matching FILTER
    fn find(&mut self, filter: &[Filter<'_>]) -> MpdResult<Vec<Song>> {
        self.send_find(filter).and_then(|()| self.read_response())
    }

    /// Search the database for songs matching FILTER (see Filters).
    /// Parameters have the same meaning as for find, except that search is not
    /// case sensitive.
    /// `ignore_diacritics` is ignored if not supported by MPD
    fn search(&mut self, filter: &[Filter<'_>], ignore_diacritics: bool) -> MpdResult<Vec<Song>> {
        if ignore_diacritics && self.supported_commands.contains("stringnormalization") {
            self.send_start_cmd_list()?;
            self.send_string_normalization_enable(&[StringNormalizationFeature::StripDiacritics])?;
            self.send_search(filter)?;
            self.send_string_normalization_disable(&[StringNormalizationFeature::StripDiacritics])?;
            self.send_execute_cmd_list()?;
            self.read_response()
        } else {
            self.send_search(filter).and_then(|()| self.read_response())
        }
    }

    fn move_in_queue(&mut self, from: SingleOrRange, to: QueuePosition) -> MpdResult<()> {
        self.send_move_in_queue(from, to).and_then(|()| self.read_ok())
    }

    fn move_id(&mut self, id: u32, to: QueuePosition) -> MpdResult<()> {
        self.send_move_id(id, to).and_then(|()| self.read_ok())
    }

    fn find_one(&mut self, filter: &[Filter<'_>]) -> MpdResult<Option<Song>> {
        let mut songs: Vec<Song> = self.send_find(filter).and_then(|()| self.read_response())?;
        Ok(songs.pop())
    }

    fn find_add(
        &mut self,
        filter: &[Filter<'_>],
        position: Option<QueuePosition>,
    ) -> MpdResult<()> {
        self.send_find_add(filter, position).and_then(|()| self.read_ok())
    }

    /// Search the database for songs matching FILTER (see Filters) AND add them
    /// to queue. Parameters have the same meaning as for find, except that
    /// search is not case sensitive.
    fn search_add(
        &mut self,
        filter: &[Filter<'_>],
        position: Option<QueuePosition>,
    ) -> MpdResult<()> {
        self.send_search_add(filter, position).and_then(|()| self.read_ok())
    }

    fn list_tag(&mut self, tag: Tag, filter: Option<&[Filter<'_>]>) -> MpdResult<MpdList> {
        self.send_list_tag(tag, filter).and_then(|()| self.read_response())
    }

    fn shuffle(&mut self, range: Option<SingleOrRange>) -> MpdResult<()> {
        self.send_shuffle(range).and_then(|()| self.read_ok())
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

        self.send_start_cmd_list()?;
        for i in 0..count {
            self.send_add(&result[i], None)?;
        }
        self.send_execute_cmd_list().and_then(|()| self.read_ok())
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

        self.send_start_cmd_list()?;
        for i in 0..count {
            let filter = &[Filter::new_with_kind(
                tag.clone(),
                std::mem::take(&mut tag_values[i]),
                FilterKind::Exact,
            )] as &[_];
            self.send_find_add(filter, None)?;
        }
        self.send_execute_cmd_list().and_then(|()| self.read_ok())
    }

    fn list_all(&mut self, path: Option<&str>) -> MpdResult<ListAll> {
        self.send_list_all(path).and_then(|()| self.read_response())
    }

    // Database
    fn lsinfo(&mut self, path: Option<&str>) -> MpdResult<LsInfo> {
        Ok(self.send_lsinfo(path).and_then(|()| self.read_opt_response())?.unwrap_or_default())
    }

    fn list_files(&mut self, path: Option<&str>) -> MpdResult<ListFiles> {
        Ok(self.send_list_files(path).and_then(|()| self.read_opt_response())?.unwrap_or_default())
    }

    fn read_picture(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>> {
        self.send_read_picture(path).and_then(|cmd| self.read_bin(&cmd))
    }

    fn albumart(&mut self, path: &str) -> MpdResult<Option<Vec<u8>>> {
        self.send_albumart(path).and_then(|cmd| self.read_bin(&cmd))
    }

    // Stored playlists
    fn list_playlists(&mut self) -> MpdResult<Vec<Playlist>> {
        self.send_list_playlists().and_then(|()| self.read_response())
    }

    fn list_playlist(&mut self, name: &str) -> MpdResult<FileList> {
        self.send_list_playlist(name).and_then(|()| self.read_response())
    }

    fn list_playlist_info(
        &mut self,
        playlist: &str,
        range: Option<SingleOrRange>,
    ) -> MpdResult<Vec<Song>> {
        self.send_list_playlist_info(playlist, range).and_then(|()| self.read_response())
    }

    fn load_playlist(&mut self, name: &str, position: Option<QueuePosition>) -> MpdResult<()> {
        self.send_load_playlist(name, position).and_then(|()| self.read_ok())
    }

    fn rename_playlist(&mut self, name: &str, new_name: &str) -> MpdResult<()> {
        self.send_rename_playlist(name, new_name).and_then(|()| self.read_ok())
    }

    fn delete_playlist(&mut self, name: &str) -> MpdResult<()> {
        self.send_delete_playlist(name).and_then(|()| self.read_ok())
    }

    fn clear_playlist(&mut self, name: &str) -> MpdResult<()> {
        self.send_delete_playlist(name).and_then(|()| self.read_ok())
    }

    fn delete_from_playlist(&mut self, name: &str, range: &SingleOrRange) -> MpdResult<()> {
        self.send_delete_from_playlist(name, range).and_then(|()| self.read_ok())
    }

    fn move_in_playlist(
        &mut self,
        playlist_name: &str,
        range: &SingleOrRange,
        target_position: usize,
    ) -> MpdResult<()> {
        self.send_move_in_playlist(playlist_name, range, target_position)
            .and_then(|()| self.read_ok())
    }

    fn add_to_playlist(
        &mut self,
        playlist_name: &str,
        uri: &str,
        target_position: Option<usize>,
    ) -> MpdResult<()> {
        self.send_add_to_playlist(playlist_name, uri, target_position).and_then(|()| self.read_ok())
    }

    fn save_queue_as_playlist(&mut self, name: &str, mode: Option<SaveMode>) -> MpdResult<()> {
        self.send_save_queue_as_playlist(name, mode).and_then(|()| self.read_ok())
    }

    // Outputs
    fn outputs(&mut self) -> MpdResult<Outputs> {
        self.send_outputs().and_then(|()| self.read_response())
    }

    fn toggle_output(&mut self, id: u32) -> MpdResult<()> {
        self.send_toggle_output(id).and_then(|()| self.read_ok())
    }

    fn enable_output(&mut self, id: u32) -> MpdResult<()> {
        self.send_enable_output(id).and_then(|()| self.read_ok())
    }

    fn disable_output(&mut self, id: u32) -> MpdResult<()> {
        self.send_disable_output(id).and_then(|()| self.read_ok())
    }

    // Decoders
    fn decoders(&mut self) -> MpdResult<Decoders> {
        self.send_decoders().and_then(|()| self.read_response())
    }

    // Stickers
    fn sticker(&mut self, uri: &str, key: &str) -> MpdResult<Option<Sticker>> {
        let result: MpdResult<Sticker> =
            self.send_sticker(uri, key).and_then(|()| self.read_response());

        if let Err(MpdError::Mpd(MpdFailureResponse { code: ErrorCode::NoExist, .. })) = result {
            return Ok(None);
        }

        result.map(Some)
    }

    fn set_sticker(&mut self, uri: &str, key: &str, value: &str) -> MpdResult<()> {
        self.send_set_sticker(uri, key, value).and_then(|()| self.read_ok())
    }

    fn delete_sticker(&mut self, uri: &str, key: &str) -> MpdResult<()> {
        self.send_delete_sticker(uri, key).and_then(|()| self.read_ok())
    }

    fn delete_all_stickers(&mut self, uri: &str) -> MpdResult<()> {
        self.send_delete_all_stickers(uri).and_then(|()| self.read_ok())
    }

    fn list_stickers(&mut self, uri: &str) -> MpdResult<Stickers> {
        self.send_list_stickers(uri).and_then(|()| self.read_response())
    }

    /// Resulting `Vec` is of the same length as input `uri`s.
    /// Default value (empty `HashMap`) is supplied if sticker
    /// for a specific URI cannot be found or an error is encountered
    fn list_stickers_multiple(&mut self, uris: &[&str]) -> MpdResult<Vec<Stickers>> {
        let mut result = Vec::with_capacity(uris.len());
        let mut list_ended_with_err = false;
        let mut i = 0;

        while i < uris.len() {
            self.send_start_cmd_list_ok()?;

            for uri in &uris[i..] {
                self.send_list_stickers(uri)?;
            }
            self.send_execute_cmd_list()?;

            for uri in &uris[i..] {
                let res: MpdResult<Stickers> = self.read_response();
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
            self.read_ok()?;
        }

        Ok(result)
    }

    fn find_stickers(
        &mut self,
        uri: &str,
        key: &str,
        filter: Option<StickerFilter>,
    ) -> MpdResult<StickersWithFile> {
        self.send_find_stickers(uri, key, filter).and_then(|()| self.read_response())
    }

    fn switch_to_partition(&mut self, name: &str) -> MpdResult<()> {
        self.send_switch_to_partition(name).and_then(|()| self.read_ok())
    }

    fn new_partition(&mut self, name: &str) -> MpdResult<()> {
        self.send_new_partition(name).and_then(|()| self.read_ok())
    }

    fn delete_partition(&mut self, name: &str) -> MpdResult<()> {
        self.send_delete_partition(name).and_then(|()| self.read_ok())
    }

    fn list_partitions(&mut self) -> MpdResult<MpdList> {
        self.send_list_partitions().and_then(|()| self.read_response())
    }

    fn move_output(&mut self, output_name: &str) -> MpdResult<()> {
        self.send_move_output(output_name).and_then(|()| self.read_ok())
    }

    fn send_message(&mut self, channel: &str, content: &str) -> MpdResult<()> {
        self.send_send_message(channel, content).and_then(|()| self.read_ok())
    }

    fn string_normalization_enable(
        &mut self,
        features: &[StringNormalizationFeature],
    ) -> MpdResult<()> {
        self.send_string_normalization_enable(features).and_then(|()| self.read_ok())
    }

    fn string_normalization_disable(
        &mut self,
        features: &[StringNormalizationFeature],
    ) -> MpdResult<()> {
        self.send_string_normalization_disable(features).and_then(|()| self.read_ok())
    }

    fn string_normalization_all(&mut self) -> MpdResult<()> {
        self.send_string_normalization_all().and_then(|()| self.read_ok())
    }

    fn string_normalization_clear(&mut self) -> MpdResult<()> {
        self.send_string_normalization_clear().and_then(|()| self.read_ok())
    }
}
