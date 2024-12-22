use anyhow::Result;
use std::path::PathBuf;

use crossbeam::channel::{Receiver, Sender};

use crate::{
    config::Config,
    shared::{
        events::{AppEvent, ClientRequest, WorkDone, WorkRequest},
        lrc::LrcIndex,
        macros::try_skip,
        mpd_query::MpdCommand,
    },
};

pub fn init(
    work_rx: Receiver<WorkRequest>,
    client_tx: Sender<ClientRequest>,
    event_tx: Sender<AppEvent>,
    config: &'static Config,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new().name("work".to_owned()).spawn(move || {
        while let Ok(req) = work_rx.recv() {
            let result = handle_work_request(req, &client_tx, config);
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
    config: &'static Config,
) -> Result<WorkDone> {
    match request {
        WorkRequest::Command(command) => {
            let callback = command.execute(config)?; // TODO log
            try_skip!(
                client_tx.send(ClientRequest::Command(MpdCommand { callback })),
                "Failed to send client request to complete command"
            );
            Ok(WorkDone::None)
        }
        WorkRequest::IndexLyrics { lyrics_dir } => {
            let start = std::time::Instant::now();
            let index = LrcIndex::index(&PathBuf::from(lyrics_dir))?;
            log::info!(found_count = index.len(), elapsed:? = start.elapsed(); "Indexed lrc files");
            Ok(WorkDone::LyricsIndexed { index })
        }
    }
}
