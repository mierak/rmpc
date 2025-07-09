use anyhow::Result;
use crossbeam::channel::Sender;
use serde::{Deserialize, Serialize};

use crate::{
    AppEvent,
    WorkRequest,
    config::Config,
    shared::ipc::SocketCommandExecute,
    ui::UiAppEvent,
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
        config: &Config,
    ) -> Result<()> {
        let tab_name = config
            .tabs
            .names
            .iter()
            .find(|name| name.as_str().eq_ignore_ascii_case(&self.tab))
            .cloned()
            .ok_or_else(|| {
                let available = config
                    .tabs
                    .names
                    .iter()
                    .map(|name| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow::anyhow!("Tab '{}' does not exist. Available tabs: {}", self.tab, available)
            })?;

        event_tx.send(AppEvent::UiEvent(UiAppEvent::ChangeTab(tab_name)))?;
        Ok(())
    }
}
