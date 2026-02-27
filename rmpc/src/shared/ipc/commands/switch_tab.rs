use anyhow::Result;
use crossbeam::channel::Sender;
use serde::{Deserialize, Serialize};

use crate::{
    AppEvent,
    WorkRequest,
    config::Config,
    shared::ipc::{IpcStream, SocketCommandExecute},
};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct SwitchTabCommand {
    pub(crate) tab: String,
}

impl SocketCommandExecute for SwitchTabCommand {
    fn execute(
        self,
        event_tx: &Sender<AppEvent>,
        _work_tx: &Sender<WorkRequest>,
        _stream: IpcStream,
        _config: &Config,
    ) -> Result<()> {
        // Skipping validation here due to config hot reload, the config passed here
        // might be stale since hot reloading only updates config in the main thread.
        // Let the main event loop handle validation with the current config.
        event_tx.send(AppEvent::RemoteSwitchTab { tab_name: self.tab })?;
        Ok(())
    }
}
