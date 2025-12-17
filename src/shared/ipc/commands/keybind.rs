use anyhow::Result;
use crossbeam::channel::Sender;
use crossterm::event::KeyEvent;
use serde::{Deserialize, Serialize};

use crate::{
    AppEvent,
    WorkRequest,
    config::{Config, keys::key::KeySequence},
    shared::ipc::{IpcStream, SocketCommandExecute},
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
        stream: IpcStream,
        _config: &Config,
    ) -> Result<()> {
        match self.key.parse::<KeySequence>() {
            Ok(seq) => {
                for key in seq {
                    let crossterm_event = KeyEvent::new(key.key, key.modifiers);
                    event_tx.send(AppEvent::UserKeyInput(crossterm_event))?;
                }
            }
            Err(err) => {
                let err = anyhow::anyhow!("Failed to parse key '{}': {}", self.key, err);
                stream.error(err.to_string());
                return Err(err);
            }
        }
        Ok(())
    }
}
