use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use index_lrc::IndexLrcCommand;
use keybind::KeybindCommand;
use query_tab::QueryCommand;
use set::SetIpcCommand;
use status_message::StatusMessageCommand;
use switch_tab::SwitchTabCommand;
use tmux::TmuxHookCommand;

use super::SocketCommand;
use crate::{
    config::cli::{RemoteCmd, SetCommand},
    shared::config_read::read_config_file,
};

pub(super) mod index_lrc;
pub mod keybind;
pub(super) mod query_tab;
pub(super) mod set;
pub(super) mod status_message;
pub mod switch_tab;
pub(super) mod tmux;

impl TryFrom<&RemoteCmd> for SocketCommand {
    type Error = anyhow::Error;

    fn try_from(value: &RemoteCmd) -> Result<Self> {
        log::debug!(value:?; "Got remote command");

        match value {
            RemoteCmd::IndexLrc { path } => {
                Ok(SocketCommand::IndexLrc(IndexLrcCommand { path: path.clone() }))
            }
            RemoteCmd::Status { message, level, timeout } => {
                Ok(SocketCommand::StatusMessage(StatusMessageCommand {
                    level: match level {
                        crate::config::cli::Level::Info => crate::shared::events::Level::Info,
                        crate::config::cli::Level::Error => crate::shared::events::Level::Error,
                        crate::config::cli::Level::Warn => crate::shared::events::Level::Warn,
                    },
                    message: message.clone(),
                    timeout: Duration::from_millis(*timeout),
                }))
            }
            RemoteCmd::Tmux { hook } => {
                Ok(SocketCommand::TmuxHook(TmuxHookCommand { hook: hook.clone() }))
            }
            RemoteCmd::Set { command: SetCommand::Config { path } } if path == "-" => {
                Ok(SocketCommand::Set(Box::new(SetIpcCommand::Config(ron::de::from_reader(
                    std::io::stdin(),
                )?))))
            }
            RemoteCmd::Set { command: SetCommand::Config { path } } => {
                let file = read_config_file(&PathBuf::from(&path))
                    .with_context(|| format!("Failed to open config file {path}"))?;
                Ok(SocketCommand::Set(Box::new(SetIpcCommand::Config(file))))
            }
            RemoteCmd::Set { command: SetCommand::Theme { path } } if path == "-" => {
                Ok(SocketCommand::Set(Box::new(SetIpcCommand::Theme(ron::de::from_reader(
                    std::io::stdin(),
                )?))))
            }
            RemoteCmd::Set { command: SetCommand::Theme { path } } => {
                let pathbuf = PathBuf::from(&path);
                let file = std::fs::File::open(&pathbuf)
                    .with_context(|| format!("Failed to open theme file {path}"))?;
                let read = std::io::BufReader::new(file);

                Ok(SocketCommand::Set(Box::new(SetIpcCommand::Theme(ron::de::from_reader(read)?))))
            }
            RemoteCmd::Keybind { key } => {
                Ok(SocketCommand::Keybind(KeybindCommand { key: key.clone() }))
            }
            RemoteCmd::SwitchTab { tab } => {
                Ok(SocketCommand::SwitchTab(SwitchTabCommand { tab: tab.clone() }))
            }
            RemoteCmd::Query { targets } => {
                Ok(SocketCommand::Query(QueryCommand { targets: targets.clone() }))
            }
        }
    }
}
