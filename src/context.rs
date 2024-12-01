use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
};

use crate::{
    config::{tabs::PaneType, Config, ImageMethod, Leak},
    mpd::{
        client::Client,
        commands::{Song, State, Status},
        mpd_client::MpdClient,
    },
    shared::{
        lrc::{Lrc, LrcIndex},
        macros::status_warn,
    },
    AppEvent, MpdCommand2, MpdCommandResult, MpdQuery, WorkRequest,
};
use anyhow::{bail, Result};

pub struct AppContext {
    pub config: &'static Config,
    pub status: Status,
    pub queue: Vec<Song>,
    pub supported_commands: HashSet<String>,
    pub app_event_sender: Sender<AppEvent>,
    pub work_sender: Sender<WorkRequest>,
    pub needs_render: AtomicBool,
    pub lrc_index: LrcIndex,
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
            lrc_index: LrcIndex::default(),
            config: config.leak(),
            status,
            queue,
            supported_commands,
            app_event_sender,
            work_sender,
            needs_render: AtomicBool::new(false),
        })
    }

    pub fn render(&self) -> Result<(), std::sync::mpsc::SendError<AppEvent>> {
        if let Ok(_) = self
            .needs_render
            .compare_exchange(true, true, Ordering::Relaxed, Ordering::Relaxed)
        {
            return Ok(());
        }

        self.app_event_sender.send(AppEvent::RequestRender(false))
    }

    pub fn finish_frame(&self) {
        self.needs_render.store(false, Ordering::Relaxed);
    }

    pub fn query(
        &self,
        id: &'static str,
        target: PaneType,
        callback: Box<dyn FnOnce(&mut Client<'_>) -> Result<MpdCommandResult> + Send>,
    ) {
        if let Err(err) = self
            .work_sender
            .send(WorkRequest::MpdQuery(MpdQuery { id, target, callback }))
        {
            log::error!(error:? = err; "Failed to send query request");
        }
    }

    pub fn command(&self, callback: Box<dyn FnOnce(&mut Client<'_>) -> Result<()> + Send>) {
        if let Err(err) = self.work_sender.send(WorkRequest::MpdCommand(MpdCommand2 { callback })) {
            log::error!(error:? = err; "Failed to send command request");
        }
    }

    pub fn find_current_song_in_queue(&self) -> Option<(usize, &Song)> {
        if self.status.state == State::Stop {
            return None;
        }

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

    pub fn find_lrc(&self) -> Result<Option<Lrc>> {
        let Some((_, song)) = self.find_current_song_in_queue() else {
            return Ok(None);
        };

        let Some(lyrics_dir) = self.config.lyrics_dir else {
            return Ok(None);
        };

        let mut path: PathBuf = PathBuf::from(lyrics_dir);
        path.push(&song.file);
        let Some(stem) = path.file_stem().map(|stem| format!("{}.lrc", stem.to_string_lossy())) else {
            bail!("No file stem for lyrics path: {path:?}");
        };

        path.pop();
        path.push(stem);
        log::debug!(path:?; "getting lrc at path");
        match std::fs::read_to_string(&path) {
            Ok(lrc) => return Ok(Some(lrc.parse()?)),
            Err(err) if matches!(err.kind(), std::io::ErrorKind::NotFound) => {
                log::trace!(path:?; "Lyrics not found");
            }
            Err(err) => {
                log::error!(err:?; "Encountered error when searching for sidecar lyrics");
            }
        }

        if let Ok(Some(lrc)) = self.lrc_index.find_lrc_for_song(song) {
            return Ok(Some(lrc));
        };

        Ok(None)
    }
}
