use std::sync::Arc;

use anyhow::Result;
use mlua::{IntoLuaMulti, Lua, LuaSerdeExt, Table, Value};
use rmpc_mpd::{
    filter::{Filter, Tag},
    mpd_client::MpdClient,
};

use crate::{async_client::AsyncClient, lua::lualib::mpd::types::Status};

mod c2c;
mod playback;
mod sticker;
pub mod types;

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
                Ok(status) => lua.to_value(&Status::from(status)).into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to get MPD status");
                    (Value::Nil, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let album_art = lua.create_async_function(move |lua, uri: String| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.albumart(&uri)).await {
                Ok(data) => data.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to get album art");
                    (Value::Nil, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let read_picture = lua.create_async_function(move |lua, uri: String| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.read_picture(&uri)).await {
                Ok(data) => data.into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to read picture");
                    (Value::Nil, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let get_song = lua.create_async_function(move |lua, uri: String| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.find_one(&[Filter::new(Tag::File, uri.as_str())])).await {
                Ok(song) => lua.to_value(&song).into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to get song by uri");
                    (Value::Nil, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let get_song_by_id = lua.create_async_function(move |lua, id: u32| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.playlist_id(id)).await {
                Ok(song) => lua.to_value(&song).into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to get song by id");
                    (Value::Nil, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    let c = Arc::clone(client);
    let get_current_song = lua.create_async_function(move |lua, ()| {
        let client = Arc::clone(&c);
        async move {
            match client.run(move |c| c.get_current_song()).await {
                Ok(song) => lua.to_value(&song).into_lua_multi(&lua),
                Err(err) => {
                    tracing::error!(err = ?err, "Failed to get current song");
                    (Value::Nil, Some(err.to_string())).into_lua_multi(&lua)
                }
            }
        }
    })?;

    mpd.raw_set("get_status", get_status)?;
    mpd.raw_set("album_art", album_art)?;
    mpd.raw_set("read_picture", read_picture)?;
    mpd.raw_set("get_song", get_song)?;
    mpd.raw_set("get_song_by_id", get_song_by_id)?;
    mpd.raw_set("get_current_song", get_current_song)?;

    Ok(mpd)
}
