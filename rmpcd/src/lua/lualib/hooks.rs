use anyhow::Result;
use mlua::{Function, Lua, Table};
use tracing::info;

const ON_SONG_CHANGE: &str = "on_song_change";
const ON_STATE_CHANGE: &str = "on_state_change";

pub fn init(lua: &Lua) -> Result<()> {
    let rmpcd = lua.globals().get::<Table>("rmpcd")?;
    let hooks = lua.create_table()?;
    rmpcd.raw_set("hooks", &hooks)?;
    hooks.raw_set(ON_SONG_CHANGE, lua.create_table()?)?;
    hooks.raw_set(ON_STATE_CHANGE, lua.create_table()?)?;

    let register = lua.create_function(|lua, (hook, func): (String, Function)| {
        let rmpcd = lua.globals().get::<Table>("rmpcd")?;
        let hooks = rmpcd.raw_get::<Table>("hooks")?;

        if !matches!(hook.as_str(), ON_SONG_CHANGE | ON_STATE_CHANGE) {
            return Err(mlua::Error::external(format!("Unknown hook type: {hook}")));
        }

        let hooks_arr = hooks.raw_get::<Table>(hook.as_str())?;

        info!(hook = %hook, "Registering hook");
        hooks_arr.raw_set(hooks_arr.raw_len() + 1, func)?;

        Ok(())
    })?;

    rmpcd.raw_set("register", register)?;

    Ok(())
}
