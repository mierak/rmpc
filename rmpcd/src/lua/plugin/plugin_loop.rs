use std::collections::HashMap;

use anyhow::Result;
use rmpc_mpd::commands::IdleEvent;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{error, trace, warn};

use crate::{
    ext::SenderExt,
    lua::{
        lualib::mpd::types::{Song, Status},
        plugin::{LuaPlugin, PluginStore, lua_plugin::PluginEvent, triggers::Triggers},
    },
};

#[derive(derive_more::Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum PluginsEvent {
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
    Messages {
        #[debug(skip)]
        messages: HashMap<String, Vec<String>>,
    },
    Idle {
        event: IdleEvent,
    },
    Shutdown,
}

pub async fn init(
    mut rx: UnboundedReceiver<PluginsEvent>,
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
            PluginsEvent::SongChange { old, new } => {
                for plugin in store.iter_with(Triggers::SongChange) {
                    plugin
                        .tx
                        .send_safe(PluginEvent::SongChange { old: old.clone(), new: new.clone() });
                }
            }
            PluginsEvent::StateChange { old, new } => {
                for plugin in store.iter_with(Triggers::StateChange) {
                    plugin
                        .tx
                        .send_safe(PluginEvent::StateChange { old: old.clone(), new: new.clone() });
                }
            }
            PluginsEvent::Messages { messages } => {
                for plugin in store.iter_with(Triggers::Message) {
                    let iter = messages
                        .iter()
                        .filter(|(channel, _)| plugin.subscribed_channels.contains(*channel));

                    for (channel, messages) in iter {
                        for message in messages {
                            plugin.tx.send_safe(PluginEvent::Message {
                                channel: channel.clone(),
                                message: message.clone(),
                            });
                        }
                    }
                }
            }
            PluginsEvent::Idle { event } => {
                for plugin in store.iter_with(Triggers::Idle) {
                    plugin.tx.send_safe(PluginEvent::Idle { event: *event });
                }
            }
            PluginsEvent::Shutdown => {
                for plugin in store.all() {
                    plugin.tx.send_safe(PluginEvent::Shutdown);
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
