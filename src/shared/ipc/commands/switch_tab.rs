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
        // tabs are case-insensitive matching
        let target_tab_name = config
            .tabs
            .names
            .iter()
            .find(|name| name.to_lowercase() == self.tab.to_lowercase())
            .cloned();

        let tab_name = match target_tab_name {
            Some(name) => name,
            None => {
                return Err(anyhow::anyhow!(
                    "Tab '{}' does not exist in configuration. Available tabs: {}",
                    self.tab,
                    config
                        .tabs
                        .names
                        .iter()
                        .map(|name| name.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        };

        event_tx.send(AppEvent::UiEvent(UiAppEvent::ChangeTab(tab_name)))?;
        Ok(())
    }
}
