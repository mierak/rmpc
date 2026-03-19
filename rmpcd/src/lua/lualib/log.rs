use anyhow::Result;
use mlua::{Lua, Table};
use tracing::{debug, error, info, trace, warn};

fn lua_location(lua: &Lua) -> (String, isize) {
    lua.inspect_stack(1, |dbg| {
        let src = dbg.source().source.map(|s| s.into_owned())?;
        let line = dbg.current_line().map_or(-1, |n| n.cast_signed());
        Some((src, line))
    })
    .flatten()
    .unwrap_or_else(|| ("<unknown>".to_string(), -1))
}

pub fn create(lua: &Lua) -> Result<Table> {
    let tbl = lua.create_table()?;

    let log_info = lua.create_function(|lua, str: String| {
        let (src, line) = lua_location(lua);
        let src = src.trim_start_matches('@');
        info!("{src}:{line}: {str}");
        Ok(())
    })?;
    let log_error = lua.create_function(|lua, str: String| {
        let (src, line) = lua_location(lua);
        let src = src.trim_start_matches('@');
        error!("{src}:{line}: {str}");
        Ok(())
    })?;
    let log_debug = lua.create_function(|lua, str: String| {
        let (src, line) = lua_location(lua);
        let src = src.trim_start_matches('@');
        debug!("{src}:{line}: {str}");
        Ok(())
    })?;
    let log_warn = lua.create_function(|lua, str: String| {
        let (src, line) = lua_location(lua);
        let src = src.trim_start_matches('@');
        warn!("{src}:{line}: {str}");
        Ok(())
    })?;
    let log_trace = lua.create_function(|lua, str: String| {
        let (src, line) = lua_location(lua);
        let src = src.trim_start_matches('@');
        trace!("{src}:{line}: {str}");
        Ok(())
    })?;

    tbl.set("info", log_info)?;
    tbl.set("error", log_error)?;
    tbl.set("debug", log_debug)?;
    tbl.set("warn", log_warn)?;
    tbl.set("trace", log_trace)?;

    Ok(tbl)
}
