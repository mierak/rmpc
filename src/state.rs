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
use tracing::instrument;

#[derive(Clone, PartialEq)]
pub struct MyVec<T>(pub Vec<T>);
impl<T> std::fmt::Debug for MyVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MyVec {{ len={} }}", self.0.len())
    }
}
impl<T> MyVec<T> {
    pub fn as_ref_mut(&mut self) -> &mut Vec<T> {
        &mut self.0
    }
}

pub struct MyVecDeque<T>(pub VecDeque<T>);
impl<T> std::fmt::Debug for MyVecDeque<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MyVecDeque {{ len={} }}", self.0.len())
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
            self.logs.0.len(),
            self.queue.as_ref().map(std::vec::Vec::len)
        )
    }
}

impl State {
    #[instrument(ret, skip_all)]
    pub async fn try_new(client: &mut Client<'_>, config: &'static Config) -> Result<Self> {
        let current_song = client.get_current_song().await?;
        let queue = client.playlist_info().await?;
        let status = client.get_status().await?;

        let album_art = if let Some(song) = queue
            .as_ref()
            .and_then(|p| p.iter().find(|s| status.songid.is_some_and(|i| i == s.id)))
        {
            client.find_album_art(&song.file).await?.map(MyVec)
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

pub trait StatusExt {
    fn bitrate(&self) -> String;
}
impl StatusExt for Status {
    fn bitrate(&self) -> String {
        match &self.bitrate {
            Some(val) => {
                if val == "0" {
                    String::new()
                } else {
                    format!(" ({val} kbps)")
                }
            }
            None => String::new(),
        }
    }
}
