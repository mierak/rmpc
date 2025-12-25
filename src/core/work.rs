use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use crossbeam::channel::{Receiver, Sender};

use crate::{
    config::{Config, cli_config::CliConfig},
    mpd::client::Client,
    shared::{
        events::{AppEvent, ClientRequest, WorkDone, WorkRequest},
        lrc::LrcIndex,
        macros::try_skip,
        mpd_client_ext::MpdClientExt,
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
        let cli_config = config.as_ref().into();
        while let Ok(req) = work_rx.recv() {
            let result = handle_work_request(req, &client_tx, &cli_config);
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
) -> Result<WorkDone> {
    match request {
        WorkRequest::SearchYt { query, kind, limit, interactive, position } => {
            if interactive {
                let items = YtDlp::search_many(kind, &query, limit)?;
                Ok(WorkDone::SearchYtResults { items, position })
            } else {
                let url = YtDlp::search_single(kind, &query)?;
                let files = YtDlp::init_and_download(config, &url)?;
                let cache_dir = config.cache_dir.clone();

                let cb = Box::new(move |client: &mut Client<'_>| {
                    client.add_downloaded_files_to_queue(files, cache_dir, position)?;
                    Ok(())
                });
                try_skip!(
                    client_tx.send(ClientRequest::Command(QueryCmd { callback: Box::new(cb) })),
                    "Failed to send client request for SearchYt"
                );
                Ok(WorkDone::None)
            }
        }
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
    }
}
