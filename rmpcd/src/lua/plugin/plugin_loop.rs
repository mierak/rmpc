use anyhow::Result;
use rmpc_mpd::commands::{IdleEvent, Status, messages::Messages};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{error, trace, warn};

use crate::{
    ext::SenderExt,
    lua::{
        lualib::mpd::types::Song,
        plugin::{LuaPlugin, PluginStore, triggers::Triggers},
    },
};

#[derive(derive_more::Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum PluginEvent {
    SongChange {
        #[debug(skip)]
        old: Option<Song>,
        #[debug(skip)]
        new: Option<Song>,
    },
    StateChange {
        #[debug(skip)]
        old: Status,
        #[debug(skip)]
        new: Status,
    },
    Message {
        #[debug(skip)]
        messages: Messages,
    },
    Idle {
        event: IdleEvent,
    },
    Shutdown,
}

pub async fn init(
    mut rx: UnboundedReceiver<PluginEvent>,
    store: PluginStore<LuaPlugin>,
) -> Result<()> {
    loop {
        trace!("Waiting for plugin events...");
        let Some(ev) = rx.recv().await else {
            warn!("Plugin task ended");
            break;
        };
        trace!("Received plugin event: {:?}", ev);

        match &ev {
            PluginEvent::SongChange { .. } => {
                for plugin in store.iter_with(Triggers::SongChange) {
                    plugin.tx.send_safe(ev.clone());
                }
            }
            PluginEvent::StateChange { .. } => {
                for plugin in store.iter_with(Triggers::StateChange) {
                    plugin.tx.send_safe(ev.clone());
                }
            }
            PluginEvent::Message { .. } => {
                for plugin in store.iter_with(Triggers::Message) {
                    plugin.tx.send_safe(ev.clone());
                }
            }
            PluginEvent::Idle { .. } => {
                for plugin in store.iter_with(Triggers::Idle) {
                    plugin.tx.send_safe(ev.clone());
                }
            }
            PluginEvent::Shutdown => {
                for plugin in store.all() {
                    plugin.tx.send_safe(ev.clone());
                }

                // Remove all plugins from the registry and join them
                for plugin in store.into_iter() {
                    trace!(path = ?plugin.path, "Shutting down plugin");
                    if let Err(err) = plugin.handle.await {
                        error!(err = ?err, path = ?plugin.path, "Failed to join plugin task");
                    }
                    trace!(path = ?plugin.path, "Plugin task shutdown successfully");
                }

                trace!("All plugins shut down, exiting plugin loop");
                break;
            }
        }
    }

    Ok(())
}
