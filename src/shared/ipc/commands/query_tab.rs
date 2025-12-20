use anyhow::Result;
use crossbeam::channel::Sender;
use serde::{Deserialize, Serialize};

use crate::{
    AppEvent,
    WorkRequest,
    config::{Config, cli::RemoteCommandQuery},
    shared::ipc::{IpcStream, SocketCommandExecute},
};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct QueryCommand {
    pub targets: Vec<RemoteCommandQuery>,
}

impl SocketCommandExecute for QueryCommand {
    fn execute(
        self,
        event_tx: &Sender<AppEvent>,
        _work_tx: &Sender<WorkRequest>,
        stream: IpcStream,
        _config: &Config,
    ) -> Result<()> {
        event_tx
            .send(AppEvent::IpcQuery { stream, targets: self.targets })
            .map_err(|err| anyhow::anyhow!("Failed to send QueryTab event: {err}"))?;
        Ok(())
    }
}
