use mlua::{Function, Lua, Table};
use tracing::info;

pub const ON_SONG_CHANGE: &str = "song_change";
pub const ON_STATE_CHANGE: &str = "state_change";
pub const ON_MESSAGES: &str = "messages";
pub const ON_MESSAGE: &str = "message";
pub const ON_IDLE: &str = "idle_event";

pub fn init(lua: &Lua) -> mlua::Result<()> {
    let rmpcd = lua.globals().get::<Table>("rmpcd")?;
    let hooks = lua.create_table()?;
    rmpcd.raw_set("hooks", &hooks)?;
    hooks.raw_set(ON_SONG_CHANGE, lua.create_table()?)?;
    hooks.raw_set(ON_STATE_CHANGE, lua.create_table()?)?;
    hooks.raw_set(ON_MESSAGES, lua.create_table()?)?;
    hooks.raw_set(ON_MESSAGE, lua.create_table()?)?;
    hooks.raw_set(ON_IDLE, lua.create_table()?)?;

    let on = lua.create_function(|lua, (hook, func): (String, Function)| {
        let rmpcd = lua.globals().get::<Table>("rmpcd")?;
        let hooks = rmpcd.raw_get::<Table>("hooks")?;

        if !matches!(
            hook.as_str(),
            ON_SONG_CHANGE | ON_STATE_CHANGE | ON_MESSAGES | ON_MESSAGE | ON_IDLE
        ) {
            return Err(mlua::Error::external(format!("Unknown hook type: {hook}")));
        }

        let hooks_arr = hooks.raw_get::<Table>(hook.as_str())?;

        info!(hook = %hook, "Registering hook");
        hooks_arr.raw_set(hooks_arr.raw_len() + 1, func)?;

        Ok(())
    })?;

    rmpcd.raw_set("on", on)?;

    Ok(())
}
