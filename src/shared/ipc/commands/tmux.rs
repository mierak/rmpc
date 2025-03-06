use anyhow::Result;
use crossbeam::channel::Sender;
use serde::{Deserialize, Serialize};

use crate::{AppEvent, WorkRequest, config::Config, shared::ipc::SocketCommandExecute};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct TmuxHookCommand {
    pub(crate) hook: String,
}

impl SocketCommandExecute for TmuxHookCommand {
    fn execute(
        self,
        event_tx: &Sender<AppEvent>,
        _work_tx: &Sender<WorkRequest>,
        _config: &Config,
    ) -> Result<()> {
        event_tx.send(AppEvent::TmuxHook { hook: self.hook })?;
        Ok(())
    }
}
