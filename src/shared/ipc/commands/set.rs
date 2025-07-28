use anyhow::Result;
use crossbeam::channel::Sender;
use serde::{Deserialize, Serialize};

use crate::{
    AppEvent,
    WorkRequest,
    config::{Config, ConfigFile, theme::UiConfigFile},
    shared::ipc::{IpcStream, SocketCommandExecute},
};

// Enum values only exist for the short time and are not constructed often, so
// the large difference should be negligible
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum SetIpcCommand {
    Config(ConfigFile),
    Theme(UiConfigFile),
}

impl SocketCommandExecute for SetIpcCommand {
    fn execute(
        self,
        event_tx: &Sender<AppEvent>,
        _work_tx: &Sender<WorkRequest>,
        _stream: IpcStream,
        _config: &Config,
    ) -> Result<()> {
        match self {
            SetIpcCommand::Config(config) => {
                let config = Box::new(config.into_config(None, None, None, None, true)?);
                Ok(event_tx.send(AppEvent::ConfigChanged { config, keep_old_theme: true })?)
            }
            SetIpcCommand::Theme(theme) => {
                let theme = Box::new(theme.try_into()?);
                Ok(event_tx.send(AppEvent::ThemeChanged { theme })?)
            }
        }
    }
}
