use std::sync::Arc;

use anyhow::Result;
use mlua::{IntoLuaMulti, Lua, Table, Value};
use rmpc_mpd::mpd_client::MpdClient;

use crate::async_client::AsyncClient;

pub fn init(lua: &Lua, mpd: &Table, client: &Arc<AsyncClient>) -> Result<()> {
    let c = Arc::clone(client);
    let set_sticker =
        lua.create_async_function(move |lua, (uri, name, value): (String, String, String)| {
            let client = Arc::clone(&c);
            async move {
                match client.run(move |c| c.set_sticker(&uri, &name, &value)).await {
                    Ok(()) => true.into_lua_multi(&lua),
                    Err(err) => {
                        tracing::error!(err = ?err, "Failed to set sticker");
                        (false, err.to_string()).into_lua_multi(&lua)
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
                    tracing::error!(err = ?err, "Failed to get sticker");
                    (Value::Nil, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    mpd.raw_set("set_song_sticker", set_sticker)?;
    mpd.raw_set("get_song_sticker", get_sticker)?;

    Ok(())
}
