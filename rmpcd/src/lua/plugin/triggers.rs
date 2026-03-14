use bitflags::bitflags;
use mlua::{FromLua, Lua};

pub const TRIGGER_COUNT: usize = 6;

bitflags! {
    #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
    pub struct Triggers: u8 {
        const SongChange = 0b00_0001;
        const StateChange = 0b00_0010;
        const Messages = 0b00_0100;
        const Message = 0b00_1000;
        const Idle = 0b01_0000;
        const Shutdown = 0b10_0000;
    }
}

impl FromLua for Triggers {
    fn from_lua(value: mlua::Value, lua: &Lua) -> mlua::Result<Self> {
        let table = mlua::Table::from_lua(value, lua)?;
        let mut result = Triggers::empty();

        for val in table.sequence_values::<String>() {
            let val = val?;
            let trigger = match val.as_str() {
                "song_change" => Ok(Triggers::SongChange),
                "state_change" => Ok(Triggers::StateChange),
                "messages" => Ok(Triggers::Messages),
                "message" => Ok(Triggers::Message),
                "idle_event" => Ok(Triggers::Idle),
                "shutdown" => Ok(Triggers::Shutdown),
                _ => Err(mlua::Error::external(format!("Unknown trigger type: {val}"))),
            };

            result |= trigger?;
        }

        Ok(result)
    }
}
