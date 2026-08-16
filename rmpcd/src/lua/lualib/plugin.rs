use std::sync::Arc;

use mlua::{Lua, Table};
use tokio::sync::RwLock;

use crate::lua::plugin::LuaPluginSpec;

pub const ON_SONG_CHANGE: &str = "song_change";
pub const ON_STATE_CHANGE: &str = "state_change";
pub const ON_MESSAGE: &str = "message";
pub const ON_IDLE: &str = "idle_event";
pub const ON_SHUTDOWN: &str = "shutdown";
pub const ON_RECONNECT: &str = "reconnect";

pub fn init(
    lua: &Lua,
    plugins: Option<&Arc<RwLock<Vec<Arc<RwLock<LuaPluginSpec>>>>>>,
) -> mlua::Result<()> {
    let rmpcd = lua.globals().get::<Table>("rmpcd")?;

    if let Some(plugins) = plugins {
        let plugins_clone = plugins.clone();

        let install = lua.create_async_function(move |lua, args: mlua::Value| {
            let p = plugins_clone.clone();
            async move { LuaPluginSpec::determine(&lua, args, &p).await }
        })?;
        rmpcd.raw_set("install", install)?;
    }

    Ok(())
}
