use std::{cell::Cell, collections::HashSet, sync::mpsc::Sender};

use crate::{
    config::{Config, ImageMethod, Leak},
    mpd::{
        client::Client,
        commands::{Song, Status},
        mpd_client::MpdClient,
    },
    shared::macros::status_warn,
    AppEvent, WorkRequest,
};
use anyhow::Result;

pub struct AppContext {
    pub config: &'static Config,
    pub status: Status,
    pub queue: Vec<Song>,
    pub supported_commands: HashSet<String>,
    pub app_event_sender: Sender<AppEvent>,
    pub work_sender: Sender<WorkRequest>,
    needs_render: Cell<bool>,
}

impl Default for AppContext {
    fn default() -> Self {
        Self {
            status: Status::default(),
            config: Box::leak(Box::default()),
            queue: Vec::default(),
            app_event_sender: std::sync::mpsc::channel().0,
            work_sender: std::sync::mpsc::channel().0,
            supported_commands: HashSet::new(),
            needs_render: Cell::new(false),
        }
    }
}

impl AppContext {
    pub fn try_new(
        client: &mut Client<'_>,
        mut config: Config,
        app_event_sender: Sender<AppEvent>,
        work_sender: Sender<WorkRequest>,
    ) -> Result<Self> {
        let status = client.get_status()?;
        let queue = client.playlist_info()?.unwrap_or_default();
        let supported_commands: HashSet<String> = client.commands()?.0.into_iter().collect();

        log::info!(supported_commands:? = supported_commands; "Supported commands by server");

        if !supported_commands.contains("albumart") || !supported_commands.contains("readpicture") {
            config.album_art.method = ImageMethod::None;
            status_warn!("Album art is disabled because it is not supported by MPD");
        }

        log::info!(config:? = config; "Resolved config");

        Ok(Self {
            config: config.leak(),
            status,
            queue,
            supported_commands,
            app_event_sender,
            work_sender,
            needs_render: Cell::new(false),
        })
    }

    pub fn render(&self) -> Result<(), std::sync::mpsc::SendError<AppEvent>> {
        if self.needs_render.get() {
            return Ok(());
        }

        self.needs_render.replace(true);
        self.app_event_sender.send(AppEvent::RequestRender(false))
    }

    pub fn finish_frame(&self) {
        self.needs_render.replace(false);
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
