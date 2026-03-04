use std::{str::FromStr, sync::Arc};

use anyhow::Result;
use mlua::{IntoLuaMulti, Lua, LuaSerdeExt, Table, Value};
use rmpc_mpd::{
    commands::{Volume, status::OnOffOneshot as MpdOnOffOneShot, volume::Bound},
    mpd_client::{MpdClient, ValueChange as MpdValueChange},
};
use serde::Deserialize;
use serde_with::DeserializeFromStr;

use crate::async_client::AsyncClient;

pub fn init(lua: &Lua, mpd: &Table, client: &Arc<AsyncClient>) -> Result<()> {
    let c = Arc::clone(client);
    let consume = lua.create_async_function(move |lua, value: Value| {
        let client = Arc::clone(&c);
        async move {
            let Ok(value): mlua::Result<OnOffOneshot> = lua.from_value(value) else {
                tracing::error!("Failed to parse value for consume");
                return (false, "Invalid consume value, expected 'on', 'off' or 'oneshot'")
                    .into_lua_multi(&lua);
            };

            match client.run(move |c| c.consume(value.into())).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to set consume");
                    (false, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let crossfade = lua.create_async_function(move |lua, seconds: u32| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.crossfade(seconds)).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to set crossfade");
                    (false, err.to_string()).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let random = lua.create_async_function(move |lua, value: bool| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.random(value)).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to set random");
                    (false, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let repeat = lua.create_async_function(move |lua, value: bool| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.repeat(value)).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to set repeat");
                    (false, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let single = lua.create_async_function(move |lua, value: Value| {
        let client = Arc::clone(&c);
        async move {
            let Ok(value): mlua::Result<OnOffOneshot> = lua.from_value(value) else {
                tracing::error!("Failed to parse value for single");
                return (false, "Invalid consume value, expected 'on', 'off' or 'oneshot'")
                    .into_lua_multi(&lua);
            };

            match client.run(move |c| c.single(value.into())).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to set single");
                    (false, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let get_volume = lua.create_async_function(move |lua, ()| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.get_volume()).await {
                Ok(vol) => vol.value().into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to get volume");
                    (Value::Nil, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let set_volume = lua.create_async_function(move |lua, volume: u32| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.set_volume(Volume::new(volume))).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to set volume");
                    (false, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let volume = lua.create_async_function(move |lua, value: String| {
        let client = Arc::clone(&c);
        async move {
            let Ok(value): Result<ValueChange> = value.parse() else {
                tracing::error!("Failed to parse volume value");
                return (false, "Invalid volume value").into_lua_multi(&lua);
            };
            match client.run(move |c| c.volume(value.into())).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to set volume");
                    (false, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let prev = lua.create_async_function(move |lua, ()| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.prev()).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to go to previous song");
                    (false, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let next = lua.create_async_function(move |lua, ()| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.next()).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to go to next song");
                    (false, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let seek_current = lua.create_async_function(move |lua, value: String| {
        let client = Arc::clone(&c);
        async move {
            let Ok(value): Result<ValueChange> = value.parse() else {
                tracing::error!("Failed to parse seek value");
                return (false, "Invalid seek value").into_lua_multi(&lua);
            };
            match client.run(move |c| c.seek_current(value.into())).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to seek current song");
                    (false, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let play = lua.create_async_function(move |lua, ()| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.play()).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to start playback");
                    (false, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let pause = lua.create_async_function(move |lua, ()| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.pause()).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to pause playback");
                    (false, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let toggle_pause = lua.create_async_function(move |lua, ()| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.pause_toggle()).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to toggle pause");
                    (false, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let stop = lua.create_async_function(move |lua, ()| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.stop()).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to stop playback");
                    (false, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    mpd.raw_set("set_consume", consume)?;
    mpd.raw_set("set_crossfade", crossfade)?;
    mpd.raw_set("set_random", random)?;
    mpd.raw_set("set_repeat", repeat)?;
    mpd.raw_set("set_single", single)?;
    mpd.raw_set("get_volume", get_volume)?;
    mpd.raw_set("set_volume", set_volume)?;
    mpd.raw_set("volume", volume)?;
    mpd.raw_set("prev", prev)?;
    mpd.raw_set("next", next)?;
    mpd.raw_set("seek_current", seek_current)?;
    mpd.raw_set("play", play)?;
    mpd.raw_set("pause", pause)?;
    mpd.raw_set("toggle_pause", toggle_pause)?;
    mpd.raw_set("stop", stop)?;

    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum OnOffOneshot {
    On,
    Off,
    Oneshot,
}

impl From<OnOffOneshot> for MpdOnOffOneShot {
    fn from(value: OnOffOneshot) -> Self {
        match value {
            OnOffOneshot::On => MpdOnOffOneShot::On,
            OnOffOneshot::Off => MpdOnOffOneShot::Off,
            OnOffOneshot::Oneshot => MpdOnOffOneShot::Oneshot,
        }
    }
}

#[derive(DeserializeFromStr)]
pub enum ValueChange {
    Increase(u32),
    Decrease(u32),
    Set(u32),
}

impl FromStr for ValueChange {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            v if v.starts_with('-') => {
                Ok(ValueChange::Decrease(v.trim_start_matches('-').parse()?))
            }
            v if v.starts_with('+') => {
                Ok(ValueChange::Increase(v.trim_start_matches('+').parse()?))
            }
            v => Ok(ValueChange::Set(v.parse()?)),
        }
    }
}

impl From<ValueChange> for MpdValueChange {
    fn from(value: ValueChange) -> Self {
        match value {
            ValueChange::Increase(v) => MpdValueChange::Increase(v),
            ValueChange::Decrease(v) => MpdValueChange::Decrease(v),
            ValueChange::Set(v) => MpdValueChange::Set(v),
        }
    }
}
