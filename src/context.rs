use crate::{
    config::Config,
    mpd::{
        client::Client,
        commands::{Song, Status},
        mpd_client::MpdClient,
    },
};
use anyhow::Result;

pub struct AppContext {
    pub config: &'static Config,
    pub status: Status,
    pub queue: Vec<Song>,
}

impl Default for AppContext {
    fn default() -> Self {
        Self {
            status: Status::default(),
            config: Box::leak(Box::default()),
            queue: Vec::default(),
        }
    }
}

impl AppContext {
    pub fn try_new(client: &mut Client<'_>, config: &'static Config) -> Result<Self> {
        let status = client.get_status()?;
        let queue = client.playlist_info()?.unwrap_or_default();

        Ok(Self { config, status, queue })
    }

    pub fn find_current_song_in_queue(&self) -> Option<&Song> {
        self.status
            .songid
            .and_then(|id| self.queue.iter().find(|song| song.id == id))
    }

    /// Gets the owned version of current song by either cloning it from queue
    /// or by querying MPD if not found
    pub fn get_current_song(&self, client: &mut impl MpdClient) -> Result<Option<Song>> {
        if let Some(song) = self.find_current_song_in_queue().cloned() {
            Ok(Some(song))
        } else {
            Ok(client.get_current_song()?)
        }
    }
}
