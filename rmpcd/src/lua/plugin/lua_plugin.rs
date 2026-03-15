use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use mlua::{IntoLua, Lua, LuaSerdeExt, Table};
use rmpc_mpd::commands::IdleEvent;
use tokio::sync::{
    RwLock,
    mpsc::{UnboundedReceiver, UnboundedSender},
};
use tracing::{debug, error, trace};

use crate::{
    async_client::AsyncClient,
    lua::{
        self,
        lualib::{
            mpd::types::{Song, State, Status},
            plugin::{ON_IDLE, ON_MESSAGE, ON_SHUTDOWN, ON_SONG_CHANGE, ON_STATE_CHANGE},
        },
        plugin::{entry::LuaPluginEntry, triggers::Triggers},
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
        channel: String,
        #[debug(skip)]
        message: String,
    },
    Idle {
        event: IdleEvent,
    },
    Shutdown,
}

#[derive(derive_more::Debug)]
pub struct LuaPlugin {
    pub path: PathBuf,
    pub triggers: Triggers,
    pub subscribed_channels: HashSet<String>,
    #[debug(skip)]
    pub tx: UnboundedSender<PluginEvent>,
    #[debug(skip)]
    pub handle: tokio::task::JoinHandle<()>,
}

impl LuaPlugin {
    pub async fn load(
        cfg_dir: &Path,
        plugin: &Arc<RwLock<LuaPluginEntry>>,
        client: &Arc<AsyncClient>,
    ) -> Result<LuaPlugin> {
        let lastfm = include_str!("../builtin/lastfm.lua");
        let notify = include_str!("../builtin/notify.lua");
        let playcount = include_str!("../builtin/playcount.lua");
        let lyrics = include_str!("../builtin/lyrics.lua");

        let plugin = plugin.read().await;
        let mut components = plugin.path.components();
        if components.next().is_some_and(|c| c.as_os_str() == "#builtin") {
            match components.next() {
                Some(c) if c.as_os_str() == "lastfm.lua" => {
                    return Self::load_single(lastfm, "lastfm", cfg_dir, &plugin, client).await;
                }
                Some(c) if c.as_os_str() == "notify.lua" => {
                    return Self::load_single(notify, "notify", cfg_dir, &plugin, client).await;
                }
                Some(c) if c.as_os_str() == "playcount.lua" => {
                    return Self::load_single(playcount, "playcount", cfg_dir, &plugin, client)
                        .await;
                }
                Some(c) if c.as_os_str() == "lyrics.lua" => {
                    return Self::load_single(lyrics, "lyrics", cfg_dir, &plugin, client).await;
                }
                c => {
                    bail!("Unknown builtin plugin: {c:?}");
                }
            }
        }

        let plugin_path = cfg_dir.join(&plugin.path);
        let content = std::fs::read(&plugin_path).with_context(|| {
            format!("Invalid or missing plugin path: {}", plugin_path.display())
        })?;

        Self::load_single(content, plugin.path.to_string_lossy().as_ref(), cfg_dir, &plugin, client)
            .await
    }

    async fn load_single(
        content: impl AsRef<[u8]>,
        name: &str,
        cfg_dir: &Path,
        plugin: &LuaPluginEntry,
        client: &Arc<AsyncClient>,
    ) -> Result<Self> {
        let lua = lua::create(cfg_dir, client, None)?;
        let state: Table = lua.load(content.as_ref()).set_name(name).eval_async().await?;

        let song_change = state.contains_key(ON_SONG_CHANGE)?;
        let state_change = state.contains_key(ON_STATE_CHANGE)?;
        let message = state.contains_key(ON_MESSAGE)?;
        let idle = state.contains_key(ON_IDLE)?;
        let shutdown = state.contains_key(ON_SHUTDOWN)?;
        let mut triggers = Triggers::empty();
        if song_change {
            triggers |= Triggers::SongChange;
        }
        if state_change {
            triggers |= Triggers::StateChange;
        }
        if message {
            triggers |= Triggers::Message;
        }
        if idle {
            triggers |= Triggers::Idle;
        }
        if shutdown {
            triggers |= Triggers::Shutdown;
        }

        if triggers.is_empty() {
            bail!("Plugin must have at least one trigger");
        }

        if let Some(setup) = state.get::<Option<mlua::Function>>("setup")? {
            let args = lua.to_value(&serde_json::from_str::<serde_json::Value>(&plugin.args)?)?;
            setup.call_async::<()>((&state, args)).await?;
        }

        let subscribed_channels = state.get::<Option<Vec<String>>>("subscribed_channels")?;
        let subscribed_channels = if let Some(channels) = subscribed_channels {
            channels.into_iter().collect()
        } else {
            HashSet::new()
        };

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = tokio::task::spawn({
            let name = name.to_string();
            async move { Self::actor_loop(name, &lua, &state, rx).await }
        });

        return Ok(Self { path: plugin.path.clone(), triggers, subscribed_channels, tx, handle });
    }

    async fn actor_loop(
        name: String,
        lua: &Lua,
        result: &mlua::Table,
        mut rx: UnboundedReceiver<PluginEvent>,
    ) {
        while let Some(event) = rx.recv().await {
            trace!(name, ?event, "Received plugin event");
            let cont = Self::handle_event(&name, lua, result, event).await;
            match cont {
                Ok(true) => {}
                Ok(false) => break,
                Err(err) => {
                    error!(err = ?err, "Error handling plugin event");
                }
            }
        }
    }

    #[inline]
    async fn handle_event(
        name: &str,
        lua: &Lua,
        state: &mlua::Table,
        event: PluginEvent,
    ) -> Result<bool> {
        match event {
            PluginEvent::SongChange { old, new } => {
                trace!(name, ON_SONG_CHANGE, "Running plugin callback");
                let old = old.clone().into_lua(lua)?;
                let new = new.clone().into_lua(lua)?;

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
                if let Err(err) = func.call_async::<()>((state, ON_IDLE, event.to_string())).await {
                    error!("Failed to call plugin callback for idle event\n{err}");
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
