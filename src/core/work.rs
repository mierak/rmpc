use std::path::PathBuf;

use anyhow::Result;
use crossbeam::channel::{Receiver, Sender};

use crate::config::Config;
use crate::shared::events::{AppEvent, ClientRequest, WorkDone, WorkRequest};
use crate::shared::lrc::LrcIndex;
use crate::shared::macros::try_skip;
use crate::shared::mpd_query::MpdCommand;

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
            let index = LrcIndex::index(&PathBuf::from(lyrics_dir));
            Ok(WorkDone::LyricsIndexed { index })
        }
    }
}
