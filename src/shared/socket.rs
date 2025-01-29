use std::path::PathBuf;

use anyhow::Result;
use crossbeam::channel::Sender;
use serde::{Deserialize, Serialize};

use super::events::Level;
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
    StatusMessage(StatusMessageCommand),
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
            SocketCommand::StatusMessage(cmd) => cmd.execute(event_tx, work_tx, config),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct StatusMessageCommand {
    pub(crate) message: String,
    pub(crate) level: Level,
}

impl SocketCommandExecute for StatusMessageCommand {
    fn execute(
        self,
        event_tx: &Sender<AppEvent>,
        _work_tx: &Sender<WorkRequest>,
        _config: &'static Config,
    ) -> Result<()> {
        event_tx.send(AppEvent::Status(self.message, self.level))?;
        Ok(())
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
