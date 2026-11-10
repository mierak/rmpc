use std::collections::HashSet;

use anyhow::Result;
use mlua::{IntoLua, Lua, LuaSerdeExt};
use rmpc_mpd::commands::IdleEvent;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, trace};

use crate::lua::{
    lualib::{
        mpd::types::{Song, State, Status},
        plugin::{
            ON_IDLE,
            ON_MESSAGE,
            ON_RECONNECT,
            ON_SHUTDOWN,
            ON_SONG_CHANGE,
            ON_START,
            ON_STATE_CHANGE,
        },
    },
    plugin::triggers::Triggers,
};

#[derive(derive_more::Debug)]
#[allow(clippy::large_enum_variant)]
pub enum PluginEvent {
    Callback {
        func: mlua::Function,
        args: Option<mlua::MultiValue>,
    },
    SocketData {
        func: mlua::Function,
        #[debug(skip)]
        data: Vec<u8>,
        #[debug(skip)]
        token: CancellationToken,
    },
    SocketClosed {
        func: mlua::Function,
        reason: Option<String>,
        #[debug(skip)]
        token: CancellationToken,
    },
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
        channel: String,
        #[debug(skip)]
        message: String,
    },
    Idle {
        event: IdleEvent,
    },
    Reconnect,
    Start,
    Shutdown,
}

#[derive(derive_more::Debug)]
pub struct LuaPlugin {
    pub name: String,
    pub triggers: Triggers,
    pub subscribed_channels: HashSet<String>,
    #[debug(skip)]
    pub tx: UnboundedSender<PluginEvent>,
    #[debug(skip)]
    pub handle: tokio::task::JoinHandle<()>,
}

impl LuaPlugin {
    pub fn new(
        name: String,
        triggers: Triggers,
        subscribed_channels: HashSet<String>,
        tx: UnboundedSender<PluginEvent>,
        lua: Lua,
        result: mlua::Table,
        mut rx: UnboundedReceiver<PluginEvent>,
    ) -> Self {
        let handle = tokio::task::spawn({
            let name = name.clone();
            async move {
                while let Some(event) = rx.recv().await {
                    trace!(name, ?event, "Received plugin event");
                    let cont = Self::handle_event(&name, &lua, &result, event).await;
                    match cont {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(err) => {
                            error!(err = ?err, "Error handling plugin event");
                        }
                    }
                }
            }
        });

        LuaPlugin { name, triggers, subscribed_channels, tx, handle }
    }

    #[inline]
    pub(crate) async fn handle_event(
        name: &str,
        lua: &Lua,
        state: &mlua::Table,
        event: PluginEvent,
    ) -> Result<bool> {
        match event {
            PluginEvent::Callback { func, args } => {
                trace!(name, "Running plugin callback");

                if let Err(err) = func.call_async::<()>(args.unwrap_or_default()).await {
                    error!("Failed to call plugin callback for song change\n{err}");
                }
            }
            PluginEvent::SocketData { func, data, token } => {
                if token.is_cancelled() {
                    trace!(name, "Skipping socket data callback, socket closed");
                    return Ok(true);
                }
                trace!(name, "Running socket data callback");

                let data = lua.create_string(&data)?;
                if let Err(err) = func.call_async::<()>(data).await {
                    error!("Failed to call socket data callback\n{err}");
                }
            }
            PluginEvent::SocketClosed { func, reason, token } => {
                if token.is_cancelled() {
                    trace!(name, "Skipping socket close callback, socket closed");
                    return Ok(true);
                }
                trace!(name, "Running socket close callback");

                if let Err(err) = func.call_async::<()>(reason).await {
                    error!("Failed to call socket close callback\n{err}");
                }
            }
            PluginEvent::SongChange { old, new } => {
                trace!(name, ON_SONG_CHANGE, "Running plugin callback");
                let old = old.into_lua(lua)?;
                let new = new.into_lua(lua)?;

                let func: mlua::Function = state.get(ON_SONG_CHANGE)?;

                if let Err(err) = func.call_async::<()>((state, old, new)).await {
                    error!("Failed to call plugin callback for song change\n{err}");
                }
            }
            PluginEvent::StateChange { old, new } => {
                trace!(name, ON_STATE_CHANGE, "Running plugin callback");
                let state_to_str = |state| match state {
                    State::Play => "play",
                    State::Pause => "pause",
                    State::Stop => "stop",
                };
                let old = lua.to_value(&state_to_str(old.state))?;
                let new = lua.to_value(&state_to_str(new.state))?;

                let func: mlua::Function = state.get(ON_STATE_CHANGE)?;

                if let Err(err) = func.call_async::<()>((state, old, new)).await {
                    error!("Failed to call plugin callback for state change\n{err}");
                }
            }
            PluginEvent::Message { channel, message } => {
                let func: Option<mlua::Function> = state.get(ON_MESSAGE)?;
                if let Some(func) = func {
                    trace!(name, ON_MESSAGE, "Running plugin callback");
                    if let Err(err) = func.call_async::<()>((state, channel, message)).await {
                        error!("Failed to call plugin on messages callback\n{err}");
                    }
                }
            }
            PluginEvent::Idle { event } => {
                trace!(name, ON_IDLE, "Running plugin callback");

                let func: mlua::Function = state.get(ON_IDLE)?;
                if let Err(err) = func.call_async::<()>((state, event.to_string())).await {
                    error!("Failed to call plugin callback for idle event\n{err}");
                }
            }
            PluginEvent::Reconnect => {
                trace!(name, ON_RECONNECT, "Running plugin callback");

                let func: mlua::Function = state.get(ON_RECONNECT)?;
                if let Err(err) = func.call_async::<()>(state).await {
                    error!("Failed to call plugin reconnect callback\n{err}");
                }
            }
            PluginEvent::Start => {
                trace!(name, ON_START, "Running plugin callback");

                let func: mlua::Function = state.get(ON_START)?;
                if let Err(err) = func.call_async::<()>(state).await {
                    error!("Failed to call plugin start callback\n{err}");
                }
            }
            PluginEvent::Shutdown => {
                let func: Option<mlua::Function> = state.get(ON_SHUTDOWN)?;

                if let Some(func) = func {
                    debug!(name, "Running plugin shutdown callback");

                    if let Err(err) = func.call_async::<()>(state).await {
                        error!("Failed to call plugin shutdown callback\n{err}");
                    }
                }
                return Ok(false);
            }
        }

        Ok(true)
    }
}
