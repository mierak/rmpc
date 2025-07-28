use std::{io::Write, os::unix::net::UnixStream, path::PathBuf, time::Duration};

use anyhow::Result;
use commands::query_tab::QueryCommand;
use crossbeam::channel::Sender;
use in_flight_ipc::{InFlightIpcCommand, IpcCommandError};
use ipc_stream::IpcStream;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use strum::IntoDiscriminant;

use super::exit_code::ExitCode;
use crate::{
    AppEvent,
    WorkRequest,
    config::{
        Config,
        cli::{RemoteCmd, RemoteCmdDiscriminants},
    },
    shared::ipc::commands::{
        index_lrc::IndexLrcCommand,
        keybind::KeybindCommand,
        set::SetIpcCommand,
        status_message::StatusMessageCommand,
        switch_tab::SwitchTabCommand,
        tmux::TmuxHookCommand,
    },
};

pub mod commands;
pub mod in_flight_ipc;
pub mod ipc_stream;

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
        stream: IpcStream,
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
    SwitchTab(SwitchTabCommand),
    Query(QueryCommand),
}

impl SocketCommandExecute for SocketCommand {
    fn execute(
        self,
        event_tx: &Sender<AppEvent>,
        work_tx: &Sender<WorkRequest>,
        stream: IpcStream,
        config: &Config,
    ) -> Result<()> {
        match self {
            SocketCommand::IndexLrc(cmd) => cmd.execute(event_tx, work_tx, stream, config),
            SocketCommand::StatusMessage(cmd) => cmd.execute(event_tx, work_tx, stream, config),
            SocketCommand::TmuxHook(cmd) => cmd.execute(event_tx, work_tx, stream, config),
            SocketCommand::Set(cmd) => cmd.execute(event_tx, work_tx, stream, config),
            SocketCommand::Keybind(cmd) => cmd.execute(event_tx, work_tx, stream, config),
            SocketCommand::SwitchTab(cmd) => cmd.execute(event_tx, work_tx, stream, config),
            SocketCommand::Query(cmd) => cmd.execute(event_tx, work_tx, stream, config),
        }
    }
}

impl RemoteCmd {
    pub(crate) fn write_to_socket(
        self,
        path: &PathBuf,
    ) -> Result<InFlightIpcCommand, IpcCommandError> {
        let cmd = SocketCommand::try_from(&self).map_err(IpcCommandError::CommandCreate)?;
        let cmd = serde_json::to_string(&cmd)?;

        let mut stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        stream.set_write_timeout(Some(Duration::from_secs(3)))?;

        stream.write_all(cmd.as_bytes())?;
        stream.write_all(b"\n")?;

        return Ok(InFlightIpcCommand { stream });
    }

    pub(crate) fn handle(self, pid: Option<u32>) -> ExitCode {
        if pid.is_none() && [RemoteCmdDiscriminants::Query].contains(&self.discriminant()) {
            match list_all_socket_paths().map(|p| p.count()) {
                Ok(1) => {} // only one path found, we don't need PID
                Ok(0) => {
                    eprintln!("No socket paths found. Please start rmpc TUI instance first");
                    return ExitCode::from(1);
                }
                Ok(_) => {
                    eprintln!("Remote command '{self}' requires a PID to be specified",);
                    return ExitCode::from(1);
                }
                Err(err) => {
                    eprintln!("Failed to list socket paths: {err}");
                    return ExitCode::from(1);
                }
            }
        }

        if let Some(pid) = pid {
            let path = get_socket_path(pid);
            self.run(&path)
        } else {
            match list_all_socket_paths() {
                Ok(paths) => {
                    let mut exit_code = ExitCode::from(0);
                    for path in paths {
                        exit_code |= self.clone().run(&path);
                    }
                    exit_code
                }
                Err(err) => {
                    eprintln!("Failed to list socket paths: {err}");
                    ExitCode::from(1)
                }
            }
        }
    }

    fn run(self, path: &PathBuf) -> ExitCode {
        let ipc = match self.write_to_socket(path) {
            Ok(ipc) => ipc,
            Err(err) => {
                eprintln!("{err}");
                return ExitCode::from(1);
            }
        };
        match ipc.read_response() {
            Ok(Some(resp)) => println!("{resp}"),
            Ok(None) => {}
            Err(err) => {
                eprintln!("{err}");
                return ExitCode::from(1);
            }
        }

        ExitCode::from(0)
    }
}
