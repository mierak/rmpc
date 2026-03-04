use std::sync::Arc;

use anyhow::Result;
use mlua::Lua;

use crate::async_client::AsyncClient;

mod c2c;
mod playback;
mod sticker;

pub fn init(lua: &Lua, client: &Arc<AsyncClient>) -> Result<()> {
    let mpd = lua.create_table()?;

    sticker::init(lua, &mpd, client)?;
    playback::init(lua, &mpd, client)?;
    c2c::init(lua, &mpd, client)?;

    lua.globals().raw_set("mpd", mpd)?;

    Ok(())
}
