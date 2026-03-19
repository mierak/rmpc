use bitflags::bitflags;
use mlua::{FromLua, Lua};

use crate::lua::lualib::plugin::{
    ON_IDLE,
    ON_MESSAGE,
    ON_RECONNECT,
    ON_SHUTDOWN,
    ON_SONG_CHANGE,
    ON_STATE_CHANGE,
};

pub const TRIGGER_COUNT: usize = 5;

bitflags! {
    #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
    pub struct Triggers: u8 {
        const SongChange = 0b000_0001;
        const StateChange = 0b000_0010;
        const Message = 0b000_1000;
        const Idle = 0b001_0000;
        const Shutdown = 0b010_0000;
        const Reconnect = 0b100_0000;
    }
}

impl FromLua for Triggers {
    fn from_lua(value: mlua::Value, lua: &Lua) -> mlua::Result<Self> {
        let table = mlua::Table::from_lua(value, lua)?;
        let mut result = Triggers::empty();

        for val in table.sequence_values::<String>() {
            let val = val?;
            let trigger = match val.as_str() {
                ON_SONG_CHANGE => Ok(Triggers::SongChange),
                ON_STATE_CHANGE => Ok(Triggers::StateChange),
                ON_MESSAGE => Ok(Triggers::Message),
                ON_IDLE => Ok(Triggers::Idle),
                ON_SHUTDOWN => Ok(Triggers::Shutdown),
                ON_RECONNECT => Ok(Triggers::Reconnect),
                _ => Err(mlua::Error::external(format!("Unknown trigger type: {val}"))),
            };

            result |= trigger?;
        }

        Ok(result)
    }
}
