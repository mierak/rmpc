use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use mlua::{IntoLua, IntoLuaMulti, Lua, Table, Value};
use rmpc_mpd::{commands::messages::Messages as MpdMessages, mpd_client::MpdClient};

use crate::async_client::AsyncClient;

pub fn init(lua: &Lua, mpd: &Table, client: &Arc<AsyncClient>) -> Result<()> {
    let c = Arc::clone(client);
    let subscribe = lua.create_async_function(move |lua, channel: String| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.subscribe(&channel)).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to subscribe to a channel");
                    (false, err.to_string()).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let unsubscribe = lua.create_async_function(move |lua, channel: String| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.unsubscribe(&channel)).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to unsubscribe from a channel");
                    (false, err.to_string()).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let channels = lua.create_async_function(move |lua, ()| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.channels()).await {
                Ok(channels) => channels.0.into_lua(&lua).into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to get subscribed channels");
                    (Value::Nil, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let send_message =
        lua.create_async_function(move |lua, (channel, message): (String, String)| {
            let client = Arc::clone(&c);
            async move {
                match client.run(move |c| c.send_message(&channel, &message)).await {
                    Ok(()) => true.into_lua_multi(&lua),
                    Err(err) => {
                        tracing::error!(err = ?err, "Failed to send message to a channel");
                        (false, err.to_string()).into_lua_multi(&lua)
                    }
                }
            }
        })?;

    let c = Arc::clone(client);
    let read_messages = lua.create_async_function(move |lua, ()| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.read_messages()).await {
                Ok(messages) => {
                    let res = Messages::from(messages);
                    res.0.into_lua(&lua).into_lua_multi(&lua)
                }
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to read messages");
                    (Value::Nil, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    mpd.raw_set("subscribe", subscribe)?;
    mpd.raw_set("unsubscribe", unsubscribe)?;
    mpd.raw_set("channels", channels)?;
    mpd.raw_set("send_message", send_message)?;
    mpd.raw_set("read_messages", read_messages)?;

    Ok(())
}

#[derive(Debug)]
struct Messages(HashMap<String, Vec<String>>);

impl From<MpdMessages> for Messages {
    fn from(value: MpdMessages) -> Self {
        Messages(value.0.into_iter().collect())
    }
}
