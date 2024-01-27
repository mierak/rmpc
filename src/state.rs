use std::collections::VecDeque;

use crate::{
    config::Config,
    mpd::{
        client::Client,
        commands::{Song, Status},
        mpd_client::MpdClient,
    },
    ui::screens::Screens,
};
use anyhow::Result;
use derive_more::{AsMut, AsRef, Deref, DerefMut};
use tracing::instrument;

#[derive(Clone, PartialEq, AsRef, AsMut, Deref)]
pub struct MyVec<T>(Vec<T>);

impl<T> MyVec<T> {
    pub fn new(value: Vec<T>) -> Self {
        Self(value)
    }
}
impl<T> std::fmt::Debug for MyVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MyVec {{ len={} }}", self.len())
    }
}

#[derive(Deref, DerefMut)]
pub struct MyVecDeque<T>(VecDeque<T>);
impl<T> std::fmt::Debug for MyVecDeque<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MyVecDeque {{ len={} }}", self.len())
    }
}

pub struct State {
    pub config: &'static Config,
    pub active_tab: Screens,
    pub status: Status,
    pub current_song: Option<Song>,
    pub queue: Option<Vec<Song>>,
    pub logs: MyVecDeque<Vec<u8>>,
    pub status_loop_active: bool,
    pub album_art: Option<MyVec<u8>>,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "State {{ active_tab: {}, logs_count: {}, queue_len: {:?}}}",
            self.active_tab,
            self.logs.len(),
            self.queue.as_ref().map(std::vec::Vec::len)
        )
    }
}

impl State {
    #[instrument(ret, skip_all)]
    pub fn try_new(client: &mut Client<'_>, config: &'static Config) -> Result<Self> {
        let current_song = client.get_current_song()?;
        let queue = client.playlist_info()?;
        let status = client.get_status()?;

        let album_art = if let Some(song) = queue
            .as_ref()
            .and_then(|p| p.iter().find(|s| status.songid.is_some_and(|i| i == s.id)))
        {
            client.find_album_art(&song.file)?.map(MyVec)
        } else {
            None
        };

        Ok(Self {
            config,
            active_tab: Screens::default(),
            status,
            queue,
            current_song,
            logs: MyVecDeque(VecDeque::new()),
            status_loop_active: false,
            album_art,
        })
    }
}

pub trait PlayListInfoExt {
    fn get_selected(&self, idx: Option<usize>) -> Option<&Song>;
    fn get_by_id(&self, id: Option<u32>) -> Option<(usize, &Song)>;
    fn is_empty_or_none(&self) -> bool;
    fn len(&self) -> Option<usize>;
}

impl PlayListInfoExt for Option<Vec<Song>> {
    fn get_selected(&self, idx: Option<usize>) -> Option<&Song> {
        match (self, idx) {
            (Some(q), Some(idx)) => q.get(idx),
            _ => None,
        }
    }

    fn get_by_id(&self, id: Option<u32>) -> Option<(usize, &Song)> {
        match (self, id) {
            (Some(q), Some(id)) => q.iter().enumerate().find(|s| s.1.id == id),
            _ => None,
        }
    }

    fn is_empty_or_none(&self) -> bool {
        match self {
            Some(v) => v.is_empty(),
            None => true,
        }
    }

    fn len(&self) -> Option<usize> {
        self.as_ref().map(std::vec::Vec::len)
    }
}
