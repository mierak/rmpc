use std::{io::Write, os::unix::net::UnixStream, path::PathBuf};

use anyhow::{Context, Result};
use crossbeam::channel::Sender;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{
    AppEvent,
    WorkRequest,
    config::{Config, cli::RemoteCmd},
    shared::ipc::commands::{
        index_lrc::IndexLrcCommand,
        keybind::KeybindCommand,
        set::SetIpcCommand,
        status_message::StatusMessageCommand,
        tmux::TmuxHookCommand,
    },
};

pub mod commands;

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
        config: &Config,
    ) -> Result<()>;
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum SocketCommand {
    IndexLrc(IndexLrcCommand),
    StatusMessage(StatusMessageCommand),
    TmuxHook(TmuxHookCommand),
    Set(Box<SetIpcCommand>),
    Keybind(KeybindCommand),
}

impl SocketCommandExecute for SocketCommand {
    fn execute(
        self,
        event_tx: &Sender<AppEvent>,
        work_tx: &Sender<WorkRequest>,
        config: &Config,
    ) -> Result<()> {
        match self {
            SocketCommand::IndexLrc(cmd) => cmd.execute(event_tx, work_tx, config),
            SocketCommand::StatusMessage(cmd) => cmd.execute(event_tx, work_tx, config),
            SocketCommand::TmuxHook(cmd) => cmd.execute(event_tx, work_tx, config),
            SocketCommand::Set(cmd) => cmd.execute(event_tx, work_tx, config),
            SocketCommand::Keybind(cmd) => cmd.execute(event_tx, work_tx, config),
        }
    }
}

impl RemoteCmd {
    pub fn write_to_socket(self, path: &PathBuf) -> Result<()> {
        let cmd = SocketCommand::try_from(self)?;
        let cmd = serde_json::to_string(&cmd).context("Failed to serialize command.")?;

        let mut stream = UnixStream::connect(path).context("Failed to connect to socket")?;
        stream.write_all(cmd.as_bytes()).context("Failed to write command to socket.")
    }
}
