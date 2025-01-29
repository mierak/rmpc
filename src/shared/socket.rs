use std::path::PathBuf;

use anyhow::Result;
use crossbeam::channel::Sender;
use serde::{Deserialize, Serialize};

use crate::{AppEvent, WorkRequest, config::Config};

pub fn get_socket_path(pid: u32) -> PathBuf {
    let mut temp = std::env::temp_dir();
    temp.push(format!("rmpc-{pid}.socket"));
    temp
}

pub(crate) trait SocketCommandExecute {
    fn execute(
        self,
        event_tx: &Sender<AppEvent>,
        work_tx: &Sender<WorkRequest>,
        config: &'static Config,
    ) -> Result<()>;
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum SocketCommand {
    IndexLrc(IndexLrcCommand),
}

impl SocketCommandExecute for SocketCommand {
    fn execute(
        self,
        event_tx: &Sender<AppEvent>,
        work_tx: &Sender<WorkRequest>,
        config: &'static Config,
    ) -> Result<()> {
        match self {
            SocketCommand::IndexLrc(cmd) => cmd.execute(event_tx, work_tx, config),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct IndexLrcCommand {
    pub(crate) path: PathBuf,
}

impl SocketCommandExecute for IndexLrcCommand {
    fn execute(
        self,
        _event_tx: &Sender<AppEvent>,
        work_tx: &Sender<WorkRequest>,
        _config: &'static Config,
    ) -> Result<()> {
        work_tx.send(WorkRequest::IndexSingleLrc { path: self.path })?;
        Ok(())
    }
}
