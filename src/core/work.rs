use std::path::PathBuf;

use anyhow::Result;
use crossbeam::channel::{Receiver, Sender};

use crate::{
    config::{Config, Leak, cli_config::CliConfig},
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
        let cli_config: CliConfig = config.into();
        let cli_config = cli_config.leak();
        while let Ok(req) = work_rx.recv() {
            let result = handle_work_request(req, &client_tx, cli_config);
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
    config: &'static CliConfig,
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
