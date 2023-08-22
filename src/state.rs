use std::collections::VecDeque;

use crate::{
    mpd::{
        client::Client,
        commands::{PlayListInfo, Song, Status},
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

pub struct MyVecDeque<T>(pub VecDeque<T>);
impl<T> std::fmt::Debug for MyVecDeque<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MyVecDeque {{ len={} }}", self.0.len())
    }
}

pub struct State {
    pub active_tab: Screens,
    pub status: Status,
    pub current_song: Option<Song>,
    pub queue: Option<PlayListInfo>,
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
            self.queue.as_ref().map(|v| v.0.len())
        )
    }
}

impl State {
    #[instrument(ret, skip_all)]
    pub async fn try_new(client: &mut Client<'_>) -> Result<Self> {
        let current_song = client.get_current_song().await?;
        let queue = client.playlist_info().await?;
        let status = client.get_status().await?;

        let album_art = if let Some(song) = queue
            .as_ref()
            .and_then(|p| p.0.iter().find(|s| status.songid.is_some_and(|i| i == s.id)))
        {
            client.find_album_art(&song.file).await.unwrap().map(MyVec)
        } else {
            None
        };

        Ok(Self {
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
