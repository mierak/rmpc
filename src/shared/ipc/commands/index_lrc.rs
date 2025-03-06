use std::path::PathBuf;

use anyhow::Result;
use crossbeam::channel::Sender;
use serde::{Deserialize, Serialize};

use crate::{AppEvent, WorkRequest, config::Config, shared::ipc::SocketCommandExecute};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct IndexLrcCommand {
    pub(crate) path: PathBuf,
}

impl SocketCommandExecute for IndexLrcCommand {
    fn execute(
        self,
        _event_tx: &Sender<AppEvent>,
        work_tx: &Sender<WorkRequest>,
        _config: &Config,
    ) -> Result<()> {
        work_tx.send(WorkRequest::IndexSingleLrc { path: self.path })?;
        Ok(())
    }
}
