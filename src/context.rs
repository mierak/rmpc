use std::sync::mpsc::Sender;

use crate::{
    config::Config,
    mpd::{
        client::Client,
        commands::{Song, Status},
        mpd_client::MpdClient,
    },
    AppEvent, WorkRequest,
};
use anyhow::Result;

pub struct AppContext {
    pub config: &'static Config,
    pub status: Status,
    pub queue: Vec<Song>,
    pub app_event_sender: Sender<AppEvent>,
    pub work_sender: Sender<WorkRequest>,
}

impl AppContext {
    pub fn try_new(
        client: &mut Client<'_>,
        config: &'static Config,
        app_event_sender: Sender<AppEvent>,
        work_request_sender: Sender<WorkRequest>,
    ) -> Result<Self> {
        let status = client.get_status()?;
        let queue = client.playlist_info()?.unwrap_or_default();

        Ok(Self {
            config,
            status,
            queue,
            app_event_sender,
            work_sender: work_request_sender,
        })
    }

    pub fn find_current_song_in_queue(&self) -> Option<(usize, &Song)> {
        self.status
            .songid
            .and_then(|id| self.queue.iter().enumerate().find(|(_, song)| song.id == id))
    }

    /// Gets the owned version of current song by either cloning it from queue
    /// or by querying MPD if not found
    pub fn get_current_song(&self, client: &mut impl MpdClient) -> Result<Option<Song>> {
        if let Some(song) = self.find_current_song_in_queue().map(|v| v.1).cloned() {
            Ok(Some(song))
        } else {
            Ok(client.get_current_song()?)
        }
    }
}
