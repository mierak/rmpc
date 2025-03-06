use anyhow::Result;
use crossbeam::channel::Sender;
use serde::{Deserialize, Serialize};

use crate::{
    AppEvent,
    WorkRequest,
    config::Config,
    shared::{events::Level, ipc::SocketCommandExecute},
};

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
        _config: &Config,
    ) -> Result<()> {
        event_tx.send(AppEvent::Status(self.message, self.level))?;
        Ok(())
    }
}
