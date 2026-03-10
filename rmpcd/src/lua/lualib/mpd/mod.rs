use std::sync::Arc;

use anyhow::Result;
use mlua::{IntoLuaMulti, Lua, LuaSerdeExt, Table, Value};
use rmpc_mpd::mpd_client::MpdClient;

use crate::async_client::AsyncClient;

mod c2c;
mod playback;
mod sticker;

pub fn create(lua: &Lua, client: &Arc<AsyncClient>) -> Result<Table> {
    let mpd = lua.create_table()?;

    sticker::init(lua, &mpd, client)?;
    playback::init(lua, &mpd, client)?;
    c2c::init(lua, &mpd, client)?;

    let c = Arc::clone(client);
    let get_status = lua.create_async_function(move |lua, ()| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.get_status()).await {
                Ok(status) => lua.to_value(&status).into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to get MPD status");
                    (Value::Nil, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    mpd.raw_set("get_status", get_status)?;

    Ok(mpd)
}
