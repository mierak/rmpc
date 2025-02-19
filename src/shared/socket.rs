use std::{io::Write, os::unix::net::UnixStream, path::PathBuf};

use anyhow::{Context, Result};
use crossbeam::channel::Sender;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::events::Level;
use crate::{
    AppEvent,
    WorkRequest,
    config::{Config, cli::NotifyCmd},
};

pub fn get_socket_path(pid: u32) -> PathBuf {
    let mut temp = std::env::temp_dir();
    temp.push(format!("rmpc-{pid}.sock"));
    temp
}

pub fn list_all_socket_paths() -> Result<impl Iterator<Item = PathBuf>> {
    let res: Vec<_> = std::fs::read_dir(std::env::temp_dir())?
        .map(|entry| -> Result<Option<_>> {
            let entry = entry?;
            let filename = entry.file_name();
            let filename = filename.to_string_lossy();
            if filename.starts_with("rmpc-") && filename.ends_with(".sock") {
                Ok(Some(entry.path()))
            } else {
                Ok(None)
            }
        })
        .filter_map(|entry| match entry {
            Ok(Some(val)) => Some(Ok(val)),
            Ok(None) => None,
            Err(err) => Some(Err(err)),
        })
        .try_collect()?;
    Ok(res.into_iter())
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
    TmuxHook(TmuxHookCommand),
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
            SocketCommand::TmuxHook(cmd) => cmd.execute(event_tx, work_tx, config),
        }
    }
}

impl NotifyCmd {
    pub fn write_to_socket(self, path: &PathBuf) -> Result<()> {
        let mut stream = UnixStream::connect(path).context("Failed to connect to socket")?;

        let cmd = Into::<SocketCommand>::into(self);
        let cmd = serde_json::to_string(&cmd).context("Failed to serialize command.")?;

        stream.write_all(cmd.as_bytes()).context("Failed to write command to socket.")
    }
}

impl From<NotifyCmd> for SocketCommand {
    fn from(value: NotifyCmd) -> Self {
        log::debug!(value:?; "Got remote command");
        match value {
            NotifyCmd::IndexLrc { ref path } => {
                SocketCommand::IndexLrc(IndexLrcCommand { path: path.clone() })
            }
            NotifyCmd::Status { ref message, level } => {
                SocketCommand::StatusMessage(StatusMessageCommand {
                    level: match level {
                        crate::config::cli::Level::Info => crate::shared::events::Level::Info,
                        crate::config::cli::Level::Error => crate::shared::events::Level::Error,
                        crate::config::cli::Level::Warn => crate::shared::events::Level::Warn,
                    },
                    message: message.clone(),
                })
            }
            NotifyCmd::Tmux { hook } => SocketCommand::TmuxHook(TmuxHookCommand { hook }),
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

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct TmuxHookCommand {
    hook: String,
}

impl SocketCommandExecute for TmuxHookCommand {
    fn execute(
        self,
        event_tx: &Sender<AppEvent>,
        _work_tx: &Sender<WorkRequest>,
        _config: &'static Config,
    ) -> Result<()> {
        event_tx.send(AppEvent::TmuxHook { hook: self.hook })?;
        Ok(())
    }
}
