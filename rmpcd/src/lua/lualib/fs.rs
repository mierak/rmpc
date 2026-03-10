use anyhow::Result;
use mlua::{ExternalError, IntoLuaMulti, Lua};
use tracing::error;

pub fn init(lua: &Lua) -> Result<()> {
    let tbl = lua.create_table()?;

    let exists =
        lua.create_async_function(async |lua, path: String| {
            match tokio::fs::try_exists(path).await {
                Ok(exists) => exists.into_lua_multi(&lua),
                Err(err) => {
                    error!(err = ?err, "Failed to check if file exists");
                    (false, err.into_lua_err()).into_lua_multi(&lua)
                }
            }
        })?;

    let create_dir_all = lua.create_async_function(async |lua, path: String| {
        match tokio::fs::create_dir_all(path).await {
            Ok(()) => true.into_lua_multi(&lua),
            Err(err) => {
                error!(err = ?err, "Failed to create directory");
                (false, err.into_lua_err()).into_lua_multi(&lua)
            }
        }
    })?;

    let create_dir = lua.create_async_function(async |lua, path: String| {
        match tokio::fs::create_dir(path).await {
            Ok(()) => true.into_lua_multi(&lua),
            Err(err) => {
                error!(err = ?err, "Failed to create directory");
                (false, err.into_lua_err()).into_lua_multi(&lua)
            }
        }
    })?;

    let write = lua.create_async_function(async |lua, (path, contents): (String, Vec<u8>)| {
        match tokio::fs::write(path, contents).await {
            Ok(()) => true.into_lua_multi(&lua),
            Err(err) => {
                error!(err = ?err, "Failed to write file");
                (false, err.into_lua_err()).into_lua_multi(&lua)
            }
        }
    })?;

    let write_str =
        lua.create_async_function(async |lua, (path, contents): (String, String)| {
            match tokio::fs::write(path, contents).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    error!(err = ?err, "Failed to write file");
                    (false, err.into_lua_err()).into_lua_multi(&lua)
                }
            }
        })?;

    let delete =
        lua.create_async_function(async |lua, path: String| {
            match tokio::fs::remove_file(path).await {
                Ok(()) => true.into_lua_multi(&lua),
                Err(err) => {
                    error!(err = ?err, "Failed to delete file");
                    (false, err.into_lua_err()).into_lua_multi(&lua)
                }
            }
        })?;

    let remove_dir = lua.create_async_function(async |lua, path: String| {
        match tokio::fs::remove_dir(path).await {
            Ok(()) => true.into_lua_multi(&lua),
            Err(err) => {
                error!(err = ?err, "Failed to delete directory");
                (false, err.into_lua_err()).into_lua_multi(&lua)
            }
        }
    })?;

    let remove_dir_all = lua.create_async_function(async |lua, path: String| {
        match tokio::fs::remove_dir_all(path).await {
            Ok(()) => true.into_lua_multi(&lua),
            Err(err) => {
                error!(err = ?err, "Failed to delete directory");
                (false, err.into_lua_err()).into_lua_multi(&lua)
            }
        }
    })?;

    tbl.raw_set("exists", exists)?;
    tbl.raw_set("create_dir_all", create_dir_all)?;
    tbl.raw_set("create_dir", create_dir)?;
    tbl.raw_set("write", write)?;
    tbl.raw_set("write_str", write_str)?;
    tbl.raw_set("delete", delete)?;
    tbl.raw_set("remove_dir", remove_dir)?;
    tbl.raw_set("remove_dir_all", remove_dir_all)?;
    lua.globals().raw_set("fs", tbl)?;

    Ok(())
}
