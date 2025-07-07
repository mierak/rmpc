use anyhow::Result;
use crossbeam::channel::Sender;
use crossterm::event::KeyEvent;
use serde::{Deserialize, Serialize};

use crate::{
    AppEvent,
    WorkRequest,
    config::{Config, keys::key::Key},
    shared::ipc::SocketCommandExecute,
};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct KeybindCommand {
    pub(crate) key: String,
}

impl SocketCommandExecute for KeybindCommand {
    fn execute(
        self,
        event_tx: &Sender<AppEvent>,
        _work_tx: &Sender<WorkRequest>,
        _config: &Config,
    ) -> Result<()> {
        // Parse the key string into a Key
        match self.key.parse::<Key>() {
            Ok(key) => {
                // Convert Key to crossterm KeyEvent
                let crossterm_event = KeyEvent::new(key.key, key.modifiers);
                // Send the key event to the application
                event_tx.send(AppEvent::UserKeyInput(crossterm_event))?;
            }
            Err(err) => {
                log::error!("Failed to parse key '{}': {}", self.key, err);
                return Err(anyhow::anyhow!("Failed to parse key '{}': {}", self.key, err));
            }
        }
        Ok(())
    }
}
