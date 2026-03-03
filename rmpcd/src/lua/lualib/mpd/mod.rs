use std::sync::Arc;

use anyhow::Result;
use mlua::{ExternalError, IntoLuaMulti, Lua, Value};
use rmpc_mpd::mpd_client::MpdClient;
use tracing::error;

use crate::async_client::AsyncClient;

pub fn init(lua: &Lua, client: &Arc<AsyncClient>) -> Result<()> {
    let mpd = lua.create_table()?;

    let c = Arc::clone(client);
    let set_sticker =
        lua.create_async_function(move |lua, (uri, name, value): (String, String, String)| {
            let client = Arc::clone(&c);
            async move {
                match client.run(move |c| c.set_sticker(&uri, &name, &value)).await {
                    Ok(()) => true.into_lua_multi(&lua),
                    Err(err) => {
                        error!(err = ?err, "Failed to set sticker");
                        (false, err.into_lua_err()).into_lua_multi(&lua)
                    }
                }
            }
        })?;

    let c = Arc::clone(client);
    let get_sticker = lua.create_async_function(move |lua, (uri, name): (String, String)| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.sticker(&uri, &name)).await {
                Ok(value) => (value.map(|s| s.value), Value::Nil).into_lua_multi(&lua),
                Err(err) => {
                    error!(err = ?err, "Failed to get sticker");
                    (Value::Nil, Some(err.into_lua_err())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    mpd.set("set_sticker", set_sticker)?;
    mpd.set("get_sticker", get_sticker)?;
    lua.globals().raw_set("mpd", mpd)?;

    Ok(())
}
