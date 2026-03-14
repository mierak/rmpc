use std::{path::PathBuf, sync::Arc};

use mlua::{ExternalResult, Lua, Table};
use tokio::sync::RwLock;

use crate::lua::plugin::LuaPluginEntry;

pub const ON_SONG_CHANGE: &str = "song_change";
pub const ON_STATE_CHANGE: &str = "state_change";
pub const ON_MESSAGES: &str = "messages";
pub const ON_MESSAGE: &str = "message";
pub const ON_IDLE: &str = "idle_event";
pub const ON_SHUTDOWN: &str = "shutdown";

pub fn init(
    lua: &Lua,
    plugins: Option<&Arc<RwLock<Vec<Arc<RwLock<LuaPluginEntry>>>>>>,
) -> mlua::Result<()> {
    let rmpcd = lua.globals().get::<Table>("rmpcd")?;

    if let Some(plugins) = plugins {
        let plugins_clone = plugins.clone();

        let install = lua.create_async_function(move |lua, args: mlua::String| {
            let p = plugins_clone.clone();
            async move {
                let mut path = PathBuf::new();
                let str = args.to_str()?;
                let split = str.split('.');

                if split.clone().count() == 0 {
                    return Err(mlua::Error::external("Plugin name cannot be empty"));
                }

                for segment in split {
                    path.push(segment);
                }
                path.set_extension("lua");

                let entry = LuaPluginEntry::new(path, String::from("{}"));
                let entry = Arc::new(RwLock::new(entry));
                let entry_clone = entry.clone();
                p.write().await.push(entry);

                let tbl = lua.create_table()?;
                let setup = lua.create_async_function(
                    move |_lua, (_self, args): (mlua::Value, mlua::Value)| {
                        let entry_clone = entry_clone.clone();
                        async move {
                            let json = serde_json::to_string(&args).into_lua_err()?;
                            entry_clone.write().await.args = json;
                            Ok(())
                        }
                    },
                )?;
                tbl.raw_set("setup", setup)?;

                Ok(tbl)
            }
        })?;
        rmpcd.raw_set("install", install)?;
    }

    Ok(())
}
