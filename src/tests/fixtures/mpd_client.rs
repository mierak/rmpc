use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Cursor},
    ops::AddAssign,
    time::Duration,
};

use itertools::Itertools;
use rstest::fixture;

use crate::mpd::{
    QueuePosition,
    commands::{
        IdleEvent,
        ListFiles,
        LsInfo,
        Playlist,
        Song,
        Status,
        Update,
        Volume,
        list::MpdList,
        list_all::ListAll,
        list_playlist::FileList,
        metadata_tag::MetadataTagExt,
        mpd_config::MpdConfig,
        status::OnOffOneshot,
        stickers::Sticker,
        volume::Bound,
    },
    errors::MpdError,
    mpd_client::{
        AlbumArtOrder,
        Filter,
        MpdClient,
        SaveMode,
        SingleOrRange,
        StickerFilter,
        StringNormalizationFeature,
        Tag,
        ValueChange,
    },
    proto_client::SocketClient,
    version::Version,
};

#[fixture]
pub fn client() -> TestMpdClient {
    let s = [
        ("artist_1", "album_1"),
        ("artist_1", "album_2"),
        ("artist_1", "album_3"),
        ("artist_2", "album_1"),
        ("artist_3", "album_1"),
        ("artist_3", "album_2"),
    ];
    let songs = s
        .iter()
        .flat_map(|(artist, album)| {
            (0..10).map(|i| Song {
                id: i,
                file: format!("{}_{}_file_{i}", *artist, *album),
                metadata: HashMap::from([
                    ("artist".to_owned(), (*artist).into()),
                    ("album".to_owned(), (*album).into()),
                    ("title".to_owned(), format!("{}_{}_file_{i}", *artist, *album).into()),
                ]),
                duration: Some(Duration::from_secs(i.into())),
                last_modified: chrono::Utc::now(),
                added: None,
            })
        })
        .collect();

    let playlists = vec![
        TestPlaylist { name: "artist_1_album_1_2".to_string(), songs_indices: (0..20).collect() },
        TestPlaylist { name: "playlist_2".to_string(), songs_indices: (10..20).collect() },
        TestPlaylist { name: "playlist_3".to_string(), songs_indices: (20..30).collect() },
        TestPlaylist { name: "playlist_4".to_string(), songs_indices: (30..40).collect() },
    ];

    TestMpdClient {
        songs,
        playlists,
        queue: Vec::new(),
        current_song_idx: None,
        volume: Volume::new(100),
        status: Status::default(),
        calls: HashMap::default(),
        rx: BufReader::new(Box::new(Cursor::new(String::new()))),
    }
}

pub struct TestPlaylist {
    pub songs_indices: Vec<usize>,
    pub name: String,
}

pub struct TestMpdClient {
    pub songs: Vec<Song>,
    pub queue: Vec<usize>,
    pub current_song_idx: Option<usize>,
    pub playlists: Vec<TestPlaylist>,
    pub volume: Volume,
    pub status: Status,
    pub calls: HashMap<String, u32>,
    pub rx: BufReader<Box<dyn BufRead>>,
}

impl TestMpdClient {
    pub fn set_read_content(&mut self, content: Box<dyn BufRead>) {
        self.rx = BufReader::new(Box::new(content));
    }

    pub fn set_read(&mut self, read: BufReader<Box<dyn BufRead>>) {
        self.rx = read;
    }
}

type MpdResult<T> = Result<T, MpdError>;
impl MpdClient for TestMpdClient {
    fn version(&mut self) -> crate::mpd::version::Version {
        todo!("Not yet implemented")
    }

    fn config(&mut self) -> Option<&MpdConfig> {
        todo!("Not yet implemented")
    }

    fn binary_limit(&mut self, _limit: u64) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn password(&mut self, _password: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn commands(&mut self) -> MpdResult<MpdList> {
        todo!("Not yet implemented")
    }

    fn not_commands(&mut self) -> MpdResult<MpdList> {
        todo!("Not yet implemented")
    }

    fn update(&mut self, _path: Option<&str>) -> MpdResult<Update> {
        todo!("Not yet implemented")
    }

    fn rescan(&mut self, _path: Option<&str>) -> MpdResult<Update> {
        todo!("Not yet implemented")
    }

    fn idle(&mut self, _subsystem: Option<IdleEvent>) -> MpdResult<Vec<IdleEvent>> {
        todo!("Not yet implemented")
    }

    fn enter_idle(&mut self, _subsystem: Option<IdleEvent>) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn noidle(&mut self) -> MpdResult<()> {
        todo!()
    }

    fn get_volume(&mut self) -> MpdResult<Volume> {
        Ok(self.volume)
    }

    fn set_volume(&mut self, volume: Volume) -> MpdResult<()> {
        self.volume = volume;
        Ok(())
    }

    fn volume(&mut self, change: ValueChange) -> MpdResult<()> {
        match change {
            ValueChange::Increase(val) => self.volume.inc_by(val),
            ValueChange::Decrease(val) => self.volume.dec_by(val),
            ValueChange::Set(val) => self.volume.set_value(val),
        };
        Ok(())
    }

    fn crossfade(&mut self, seconds: u32) -> MpdResult<()> {
        self.status.xfade = Some(seconds);
        Ok(())
    }

    fn get_current_song(&mut self) -> MpdResult<Option<Song>> {
        Ok(self.current_song_idx.and_then(|idx| self.songs.get(idx).cloned()))
    }

    fn get_status(&mut self) -> MpdResult<Status> {
        Ok(self.status.clone())
    }

    fn pause_toggle(&mut self) -> MpdResult<()> {
        use crate::mpd::commands::State as S;
        self.status.state = match self.status.state {
            S::Play => S::Pause,
            S::Stop => S::Stop,
            S::Pause => S::Play,
        };
        Ok(())
    }

    fn pause(&mut self) -> MpdResult<()> {
        use crate::mpd::commands::State as S;
        self.status.state = S::Pause;
        Ok(())
    }

    fn unpause(&mut self) -> MpdResult<()> {
        use crate::mpd::commands::State as S;
        self.status.state = S::Play;
        Ok(())
    }

    fn next(&mut self) -> MpdResult<()> {
        self.current_song_idx = self.current_song_idx.map(|idx| (idx + 1) % self.queue.len());
        Ok(())
    }

    fn prev(&mut self) -> MpdResult<()> {
        self.current_song_idx = self.current_song_idx.map(|idx| match idx {
            0 => self.queue.len() - 1,
            _ => idx - 1,
        });
        Ok(())
    }

    fn play_pos(&mut self, pos: usize) -> MpdResult<()> {
        if (pos) < self.queue.len() {
            self.current_song_idx = Some(pos);
            self.status.state = crate::mpd::commands::State::Play;
            Ok(())
        } else {
            Err(MpdError::Generic("Invalid song index".to_string()))
        }
    }

    fn play(&mut self) -> MpdResult<()> {
        self.status.state = crate::mpd::commands::State::Play;
        Ok(())
    }

    fn play_id(&mut self, id: u32) -> MpdResult<()> {
        match self.queue.iter().enumerate().find(|(_idx, s)| self.songs[**s].id == id) {
            Some((idx, _)) => {
                self.current_song_idx = Some(idx);
                Ok(())
            }
            None => Err(MpdError::Generic("Song id not found".to_string())),
        }
    }

    fn stop(&mut self) -> MpdResult<()> {
        self.status.state = crate::mpd::commands::State::Stop;
        Ok(())
    }

    fn seek_current(&mut self, _value: ValueChange) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn repeat(&mut self, enabled: bool) -> MpdResult<()> {
        self.status.repeat = enabled;
        Ok(())
    }

    fn random(&mut self, enabled: bool) -> MpdResult<()> {
        self.status.random = enabled;
        Ok(())
    }

    fn single(&mut self, single: OnOffOneshot) -> MpdResult<()> {
        self.status.single = single;
        Ok(())
    }

    fn consume(&mut self, consume: OnOffOneshot) -> MpdResult<()> {
        self.status.consume = consume;
        Ok(())
    }

    fn mount(&mut self, _name: &str, _path: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn unmount(&mut self, _name: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn list_mounts(&mut self) -> MpdResult<crate::mpd::commands::Mounts> {
        todo!("Not yet implemented")
    }

    fn add(&mut self, _path: &str, _position: Option<QueuePosition>) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn clear(&mut self) -> MpdResult<()> {
        self.songs.clear();
        self.current_song_idx = None;
        self.status.state = crate::mpd::commands::State::Stop;
        Ok(())
    }

    fn swap_position(&mut self, _song1: usize, _song2: usize) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn swap_id(&mut self, _id1: u32, _id2: u32) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn delete_id(&mut self, _id: u32) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn delete_from_queue(&mut self, _songs: SingleOrRange) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn playlist_info(&mut self) -> MpdResult<Option<Vec<Song>>> {
        Ok(Some(self.queue.iter().map(|idx| self.songs[*idx].clone()).collect_vec()))
    }

    /// `FilterKind` not implemented, everything is treated as Contains
    fn find(&mut self, filter: &[Filter<'_>]) -> MpdResult<Vec<Song>> {
        Ok(self
            .songs
            .iter()
            .filter(|s| {
                let mut matches = true;
                let albumartist = s.metadata.get("albumartist");
                let genre = s.metadata.get("genre");
                let album = s.metadata.get("album");
                let artist = s.metadata.get("artist");
                let title = s.metadata.get("title");
                let values = [
                    artist.last(),
                    albumartist.last(),
                    album.last(),
                    title.last(),
                    Some(&s.file),
                    genre.last(),
                ];

                for filter in filter {
                    let value = match filter.tag {
                        Tag::Any => values
                            .iter()
                            .any(|a| a.is_some_and(|a| a.contains(filter.value.as_ref()))),
                        Tag::Artist => values[0].is_some_and(|a| a.contains(filter.value.as_ref())),
                        Tag::AlbumArtist => {
                            values[1].is_some_and(|a| a.contains(filter.value.as_ref()))
                        }
                        Tag::Album => values[2].is_some_and(|a| a.contains(filter.value.as_ref())),
                        Tag::Title => values[3].is_some_and(|a| a.contains(filter.value.as_ref())),
                        Tag::File => values[4].is_some_and(|a| a.contains(filter.value.as_ref())),
                        Tag::Genre => values[5].is_some_and(|a| a.contains(filter.value.as_ref())),
                        Tag::Custom(_) => false,
                    };
                    if !value {
                        matches = false;
                        break;
                    }
                }
                matches
            })
            .cloned()
            .collect())
    }

    fn search(&mut self, filter: &[Filter<'_>], _ignore_diacritics: bool) -> MpdResult<Vec<Song>> {
        Ok(self
            .songs
            .iter()
            .filter(|s| {
                let mut matches = true;
                let albumartist = s.metadata.get("albumartist");
                let genre = s.metadata.get("genre");
                let album = s.metadata.get("album");
                let artist = s.metadata.get("artist");
                let title = s.metadata.get("title");
                let values = [
                    artist.last(),
                    albumartist.last(),
                    album.last(),
                    title.last(),
                    Some(&s.file),
                    genre.last(),
                ];

                for filter in filter {
                    let value = match filter.tag {
                        Tag::Any => values.iter().any(|a| {
                            a.is_some_and(|a| {
                                a.to_lowercase().contains(&filter.value.to_lowercase())
                            })
                        }),
                        Tag::Artist => values[0].is_some_and(|a| {
                            a.to_lowercase().contains(&filter.value.to_lowercase())
                        }),
                        Tag::AlbumArtist => values[1].is_some_and(|a| {
                            a.to_lowercase().contains(&filter.value.to_lowercase())
                        }),
                        Tag::Album => values[2].is_some_and(|a| {
                            a.to_lowercase().contains(&filter.value.to_lowercase())
                        }),
                        Tag::Title => values[3].is_some_and(|a| {
                            a.to_lowercase().contains(&filter.value.to_lowercase())
                        }),
                        Tag::File => values[4].is_some_and(|a| {
                            a.to_lowercase().contains(&filter.value.to_lowercase())
                        }),
                        Tag::Genre => values[5].is_some_and(|a| {
                            a.to_lowercase().contains(&filter.value.to_lowercase())
                        }),
                        Tag::Custom(_) => false,
                    };
                    if !value {
                        matches = false;
                        break;
                    }
                }
                matches
            })
            .cloned()
            .collect())
    }

    fn move_in_queue(&mut self, _from: SingleOrRange, _to: QueuePosition) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn move_id(&mut self, _id: u32, _to: QueuePosition) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn find_one(&mut self, filter: &[Filter<'_>]) -> MpdResult<Option<Song>> {
        let mut res = self.find(filter)?;
        if res.len() > 1 {
            Err(MpdError::Generic("More than one song found".to_string()))
        } else {
            Ok(Some(res.remove(0)))
        }
    }

    fn find_add(
        &mut self,
        _filter: &[Filter<'_>],
        _position: Option<QueuePosition>,
    ) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn search_add(
        &mut self,
        _filter: &[Filter<'_>],
        _position: Option<QueuePosition>,
    ) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn list_tag(&mut self, _tag: Tag, _filter: Option<&[Filter<'_>]>) -> MpdResult<MpdList> {
        todo!("Not yet implemented")
    }

    fn shuffle(&mut self, _range: Option<SingleOrRange>) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn add_random_songs(&mut self, _: usize, _: Option<&[Filter<'_>]>) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn add_random_tag(&mut self, _: usize, _: Tag) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn list_all(&mut self, _: Option<&str>) -> MpdResult<ListAll> {
        todo!("Not yet implemented")
    }

    fn lsinfo(&mut self, _path: Option<&str>) -> MpdResult<LsInfo> {
        todo!("Not yet implemented")
    }

    fn list_files(&mut self, _path: Option<&str>) -> MpdResult<ListFiles> {
        todo!("Not yet implemented")
    }

    fn read_picture(&mut self, _path: &str) -> MpdResult<Option<Vec<u8>>> {
        todo!("Not yet implemented")
    }

    fn albumart(&mut self, _path: &str) -> MpdResult<Option<Vec<u8>>> {
        todo!("Not yet implemented")
    }

    fn list_playlists(&mut self) -> MpdResult<Vec<Playlist>> {
        self.playlists
            .iter()
            .map(|p| Ok(Playlist { name: p.name.clone(), last_modified: "2021-01-01".to_string() }))
            .collect()
    }

    fn list_playlist(&mut self, name: &str) -> MpdResult<FileList> {
        self.playlists.iter().find(|p| p.name == name).map_or_else(
            || Err(MpdError::Generic("Playlist not found".to_string())),
            |p| {
                Ok(FileList(
                    p.songs_indices.iter().map(|idx| self.songs[*idx].file.clone()).collect(),
                ))
            },
        )
    }

    fn list_playlist_info(
        &mut self,
        playlist: &str,
        _range: Option<SingleOrRange>,
    ) -> MpdResult<Vec<Song>> {
        self.playlists.iter().find(|p| p.name == playlist).map_or_else(
            || Err(MpdError::Generic("Playlist not found".to_string())),
            |p| {
                Ok(p.songs_indices
                    .iter()
                    .map(|idx| Song {
                        file: self.songs[*idx].file.clone(),
                        id: *idx as u32,
                        duration: None,
                        metadata: HashMap::default(),
                        last_modified: chrono::Utc::now(),
                        added: None,
                    })
                    .collect())
            },
        )
    }

    fn load_playlist(&mut self, _name: &str, _position: Option<QueuePosition>) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn rename_playlist(&mut self, _name: &str, _new_name: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn delete_playlist(&mut self, _name: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn clear_playlist(&mut self, _name: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn delete_from_playlist(
        &mut self,
        _playlist_name: &str,
        _songs: &SingleOrRange,
    ) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn move_in_playlist(
        &mut self,
        _playlist_name: &str,
        _range: &SingleOrRange,
        _target_position: usize,
    ) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn add_to_playlist(
        &mut self,
        _playlist_name: &str,
        _uri: &str,
        _target_position: Option<usize>,
    ) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn save_queue_as_playlist(&mut self, _name: &str, _mode: Option<SaveMode>) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn find_album_art(&mut self, _path: &str, _order: AlbumArtOrder) -> MpdResult<Option<Vec<u8>>> {
        self.calls.entry("find_album_art".to_string()).or_default().add_assign(1);
        Ok(Some(Vec::new()))
    }

    fn outputs(&mut self) -> MpdResult<crate::mpd::commands::outputs::Outputs> {
        todo!("Not yet implemented")
    }

    fn toggle_output(&mut self, _id: u32) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn enable_output(&mut self, _id: u32) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn disable_output(&mut self, _id: u32) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn decoders(&mut self) -> MpdResult<crate::mpd::commands::decoders::Decoders> {
        todo!("Not yet implemented")
    }

    fn sticker(&mut self, _uri: &str, _name: &str) -> MpdResult<Option<Sticker>> {
        todo!("Not yet implemented")
    }

    fn set_sticker(&mut self, _uri: &str, _name: &str, _value: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn delete_sticker(&mut self, _uri: &str, _name: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn delete_all_stickers(&mut self, _uri: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn list_stickers(&mut self, _uri: &str) -> MpdResult<crate::mpd::commands::stickers::Stickers> {
        todo!("Not yet implemented")
    }

    fn list_stickers_multiple(
        &mut self,
        _uris: &[&str],
    ) -> MpdResult<Vec<crate::mpd::commands::stickers::Stickers>> {
        todo!("Not yet implemented")
    }

    fn find_stickers(
        &mut self,
        _uri: &str,
        _name: &str,
        _filter: Option<StickerFilter>,
    ) -> MpdResult<crate::mpd::commands::stickers::StickersWithFile> {
        todo!("Not yet implemented")
    }

    fn switch_to_partition(&mut self, _name: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn new_partition(&mut self, _name: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn delete_partition(&mut self, _name: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn list_partitions(&mut self) -> MpdResult<MpdList> {
        todo!("Not yet implemented")
    }

    fn move_output(&mut self, _output_name: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn send_message(&mut self, _channel: &str, _content: &str) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn string_normalization_enable(
        &mut self,
        _features: &[StringNormalizationFeature],
    ) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn string_normalization_disable(
        &mut self,
        _features: &[StringNormalizationFeature],
    ) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn string_normalization_all(&mut self) -> MpdResult<()> {
        todo!("Not yet implemented")
    }

    fn string_normalization_clear(&mut self) -> MpdResult<()> {
        todo!("Not yet implemented")
    }
}

impl SocketClient for TestMpdClient {
    fn write(&mut self, _bytes: &[u8]) -> std::io::Result<()> {
        Ok(())
    }

    fn read(&mut self) -> &mut impl BufRead {
        &mut self.rx
    }

    fn clear_read_buf(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn version(&self) -> crate::mpd::version::Version {
        Version::new(0, 0, 0)
    }
}
