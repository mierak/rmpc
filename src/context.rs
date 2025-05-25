use std::{cell::Cell, collections::HashSet, ops::AddAssign, time::Instant};

use anyhow::{Context, Result, bail};
use bon::bon;
use crossbeam::channel::{SendError, Sender, bounded};

use crate::{
    AppEvent,
    MpdCommand,
    MpdQuery,
    MpdQueryResult,
    WorkRequest,
    config::{
        Config,
        album_art::ImageMethod,
        tabs::{PaneType, TabName},
    },
    core::scheduler::{Scheduler, time_provider::DefaultTimeProvider},
    mpd::{
        client::Client,
        commands::{Song, State, Status},
        mpd_client::MpdClient,
    },
    shared::{
        events::ClientRequest,
        lrc::{Lrc, LrcIndex, get_lrc_path},
        macros::status_warn,
        mpd_query::MpdQuerySync,
    },
};

#[derive(derive_more::Debug)]
pub struct AppContext {
    pub(crate) config: std::sync::Arc<Config>,
    pub(crate) status: Status,
    pub(crate) queue: Vec<Song>,
    pub(crate) active_tab: TabName,
    pub(crate) supported_commands: HashSet<String>,
    pub(crate) db_update_start: Option<Instant>,
    #[debug(skip)]
    pub(crate) app_event_sender: Sender<AppEvent>,
    #[debug(skip)]
    pub(crate) work_sender: Sender<WorkRequest>,
    #[debug(skip)]
    pub(crate) client_request_sender: Sender<ClientRequest>,
    pub(crate) needs_render: Cell<bool>,
    #[debug(skip)]
    pub(crate) lrc_index: LrcIndex,
    pub(crate) rendered_frames: u64,
    pub(crate) should_fetch_stickers: bool,
    #[debug(skip)]
    pub(crate) scheduler: Scheduler<(Sender<AppEvent>, Sender<ClientRequest>), DefaultTimeProvider>,
}

#[bon]
impl AppContext {
    pub(crate) fn try_new(
        client: &mut Client<'_>,
        mut config: Config,
        app_event_sender: Sender<AppEvent>,
        work_sender: Sender<WorkRequest>,
        client_request_sender: Sender<ClientRequest>,
        mut scheduler: Scheduler<(Sender<AppEvent>, Sender<ClientRequest>), DefaultTimeProvider>,
    ) -> Result<Self> {
        let supported_commands: HashSet<String> = client.commands()?.0.into_iter().collect();
        let sticker_support_needed = config.sticker_support_needed();
        log::info!(supported_commands:? = supported_commands, sticker_support_needed; "Supported commands by server");

        if sticker_support_needed && !supported_commands.contains("sticker") {
            bail!(
                "Rmpc was configured to display stickers but MPD did not report sticker support.\nCheck if you have 'sticker_file' configured your in mpd.conf."
            );
        }

        let status = client.get_status()?;
        let queue = client.playlist_info(sticker_support_needed)?.unwrap_or_default();

        if !supported_commands.contains("albumart") || !supported_commands.contains("readpicture") {
            config.album_art.method = ImageMethod::None;
            status_warn!("Album art is disabled because it is not supported by MPD");
        }

        log::info!(config:? = config; "Resolved config");

        let active_tab = config.tabs.names.first().context("Expected at least one tab")?.clone();
        scheduler.start();
        Ok(Self {
            lrc_index: LrcIndex::default(),
            config: std::sync::Arc::new(config),
            status,
            queue,
            active_tab,
            supported_commands,
            db_update_start: None,
            app_event_sender,
            work_sender,
            scheduler,
            client_request_sender,
            needs_render: Cell::new(false),
            should_fetch_stickers: sticker_support_needed,
            rendered_frames: 0,
        })
    }

    pub(crate) fn render(&self) -> Result<(), SendError<AppEvent>> {
        if self.needs_render.get() {
            return Ok(());
        }

        self.needs_render.replace(true);
        self.app_event_sender.send(AppEvent::RequestRender)
    }

    pub(crate) fn finish_frame(&mut self) {
        self.needs_render.replace(false);
        self.rendered_frames.add_assign(1);
    }

    pub(crate) fn query_sync<T: Send + Sync + 'static>(
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
    pub(crate) fn query(
        &self,
        #[builder(finish_fn)] on_done: impl FnOnce(&mut Client<'_>) -> Result<MpdQueryResult>
        + Send
        + 'static,
        id: &'static str,
        target: Option<PaneType>,
        replace_id: Option<&'static str>,
    ) {
        let query = MpdQuery { id, target, replace_id, callback: Box::new(on_done) };
        if let Err(err) = self.client_request_sender.send(ClientRequest::Query(query)) {
            log::error!(error:? = err; "Failed to send query request");
        }
    }

    pub(crate) fn command(
        &self,
        callback: impl FnOnce(&mut Client<'_>) -> Result<()> + Send + 'static,
    ) {
        if let Err(err) = self
            .client_request_sender
            .send(ClientRequest::Command(MpdCommand { callback: Box::new(callback) }))
        {
            log::error!(error:? = err; "Failed to send command request");
        }
    }

    pub(crate) fn find_current_song_in_queue(&self) -> Option<(usize, &Song)> {
        if self.status.state == State::Stop {
            return None;
        }

        self.status
            .songid
            .and_then(|id| self.queue.iter().enumerate().find(|(_, song)| song.id == id))
    }

    pub(crate) fn find_lrc(&self) -> Result<Option<Lrc>> {
        let Some((_, song)) = self.find_current_song_in_queue() else {
            return Ok(None);
        };

        let Some(lyrics_dir) = &self.config.lyrics_dir else {
            return Ok(None);
        };

        let path = get_lrc_path(lyrics_dir, &song.file)?;
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
        }

        Ok(None)
    }
}

impl Config {
    fn sticker_support_needed(&self) -> bool {
        self.theme.song_table_format.iter().any(|column| column.prop.kind.contains_stickers())
            || self.theme.browser_song_format.0.iter().any(|prop| prop.kind.contains_stickers())
            || self.theme.header.rows.iter().any(|row| {
                row.left.iter().any(|left| left.kind.contains_stickers())
                    || row.center.iter().any(|center| center.kind.contains_stickers())
                    || row.right.iter().any(|right| right.kind.contains_stickers())
            })
    }
}
