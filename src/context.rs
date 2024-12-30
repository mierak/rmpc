use std::{cell::Cell, collections::HashSet, path::PathBuf};

use crate::{
    config::{tabs::PaneType, Config, ImageMethod, Leak},
    mpd::{
        client::Client,
        commands::{Song, State, Status},
        mpd_client::MpdClient,
    },
    shared::{
        events::ClientRequest,
        lrc::{Lrc, LrcIndex},
        macros::status_warn,
        mpd_query::MpdQuerySync,
    },
    AppEvent, MpdCommand, MpdQuery, MpdQueryResult, WorkRequest,
};
use anyhow::{bail, Result};
use bon::bon;
use crossbeam::channel::{bounded, SendError, Sender};

pub struct AppContext {
    pub config: &'static Config,
    pub status: Status,
    pub queue: Vec<Song>,
    pub supported_commands: HashSet<String>,
    pub app_event_sender: Sender<AppEvent>,
    pub work_sender: Sender<WorkRequest>,
    pub client_request_sender: Sender<ClientRequest>,
    pub needs_render: Cell<bool>,
    pub lrc_index: LrcIndex,
}

#[bon]
impl AppContext {
    pub fn try_new(
        client: &mut Client<'_>,
        mut config: Config,
        app_event_sender: Sender<AppEvent>,
        work_sender: Sender<WorkRequest>,
        client_request_sender: Sender<ClientRequest>,
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
            client_request_sender,
            needs_render: Cell::new(false),
        })
    }

    pub fn render(&self) -> Result<(), SendError<AppEvent>> {
        if self.needs_render.get() {
            return Ok(());
        }

        self.needs_render.replace(true);
        self.app_event_sender.send(AppEvent::RequestRender)
    }

    pub fn finish_frame(&self) {
        self.needs_render.replace(false);
    }

    pub fn query_sync<T: Send + Sync + 'static>(
        &self,
        on_done: impl FnOnce(&mut Client<'_>) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (tx, rx) = bounded(1);
        let query = MpdQuerySync {
            callback: Box::new(|client| Ok(MpdQueryResult::Any(Box::new((on_done)(client)?)))),
            tx,
        };

        if let Err(err) = self.client_request_sender.send(ClientRequest::QuerySync(query)) {
            log::error!(error:? = err; "Failed to send query request");
            bail!("Failed to send sync query request");
        }

        if let MpdQueryResult::Any(any) = rx.recv()? {
            if let Ok(val) = any.downcast::<T>() {
                return Ok(*val);
            }
            bail!("Received unknown type answer for sync query request",);
        }

        bail!("Received unknown MpdQueryResult for sync query request");
    }

    #[builder(finish_fn(name = query))]
    pub fn query(
        &self,
        #[builder(finish_fn)] on_done: impl FnOnce(&mut Client<'_>) -> Result<MpdQueryResult> + Send + 'static,
        id: &'static str,
        target: Option<PaneType>,
        replace_id: Option<&'static str>,
    ) {
        let query = MpdQuery {
            id,
            target,
            replace_id,
            callback: Box::new(on_done),
        };
        if let Err(err) = self.client_request_sender.send(ClientRequest::Query(query)) {
            log::error!(error:? = err; "Failed to send query request");
        }
    }

    pub fn command(&self, callback: impl FnOnce(&mut Client<'_>) -> Result<()> + Send + 'static) {
        if let Err(err) = self.client_request_sender.send(ClientRequest::Command(MpdCommand {
            callback: Box::new(callback),
        })) {
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
