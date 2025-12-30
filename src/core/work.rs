use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use crossbeam::channel::{Receiver, Sender};

use crate::{
    config::{Config, cli_config::CliConfig},
    shared::{
        events::{AppEvent, ClientRequest, WorkDone, WorkRequest},
        lrc::LrcIndex,
        macros::try_skip,
        mpd_query::MpdCommand as QueryCmd,
        ytdlp::YtDlp,
    },
};

pub fn init(
    work_rx: Receiver<WorkRequest>,
    client_tx: Sender<ClientRequest>,
    event_tx: Sender<AppEvent>,
    config: Arc<Config>,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new().name("work".to_owned()).spawn(move || {
        let ytdlp = config.cache_dir.as_ref().map(|dir| YtDlp::new(dir.clone()));
        let cli_config = config.as_ref().into();
        while let Ok(req) = work_rx.recv() {
            let result = handle_work_request(req, &client_tx, &cli_config, ytdlp.as_ref());
            try_skip!(
                event_tx.send(AppEvent::WorkDone(result)),
                "Failed to send work done notification"
            );
        }
    })
}

fn handle_work_request(
    request: WorkRequest,
    client_tx: &Sender<ClientRequest>,
    config: &CliConfig,
    ytdlp: Option<&YtDlp>,
) -> Result<WorkDone> {
    match request {
        WorkRequest::Command(command) => {
            let callback = command.execute(config)?; // TODO log
            try_skip!(
                client_tx.send(ClientRequest::Command(QueryCmd { callback })),
                "Failed to send client request to complete command"
            );
            Ok(WorkDone::None)
        }
        WorkRequest::IndexLyrics { lyrics_dir } => {
            let index = LrcIndex::index(&PathBuf::from(lyrics_dir));
            Ok(WorkDone::LyricsIndexed { index })
        }
        WorkRequest::IndexSingleLrc { path } => {
            let metadata = LrcIndex::index_single(&path)?;
            Ok(WorkDone::SingleLrcIndexed { path, metadata })
        }
        WorkRequest::ResizeImage(fn_once) => Ok(WorkDone::ImageResized { data: fn_once() }),
        WorkRequest::SearchYt { query, kind, limit, interactive, position } => {
            if ytdlp.is_none() {
                anyhow::bail!("Youtube support requires 'cache_dir' to be configured")
            }

            let limit = if interactive { limit } else { 1 };
            let items = YtDlp::search(kind, &query, limit)?;

            Ok(WorkDone::SearchYtResults { items, position, interactive })
        }
        WorkRequest::YtDlpDownload { id, url } => {
            let Some(ytdlp) = ytdlp else {
                anyhow::bail!("Youtube support requires 'cache_dir' to be configured")
            };

            let result = ytdlp.download_single(&url);
            Ok(WorkDone::YtDlpDownloaded { id, result })
        }
        WorkRequest::YtDlpResolvePlaylist { playlist } => {
            let Some(ytdlp) = ytdlp else {
                anyhow::bail!("Youtube support requires 'cache_dir' to be configured")
            };

            let result = ytdlp.resolve_playlist_urls(&playlist)?;
            Ok(WorkDone::YtDlpPlaylistResolved { urls: result })
        }
    }
}
