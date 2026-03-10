use anyhow::Result;
use mlua::{Lua, Table};
use tracing::{debug, error, info, trace, warn};

pub fn create(lua: &Lua) -> Result<Table> {
    let tbl = lua.create_table()?;

    let log_info = lua.create_function(|_, str: String| {
        info!("{str}");
        Ok(())
    })?;
    let log_error = lua.create_function(|_, str: String| {
        error!("{str}");
        Ok(())
    })?;
    let log_debug = lua.create_function(|_, str: String| {
        debug!("{str}");
        Ok(())
    })?;
    let log_warn = lua.create_function(|_, str: String| {
        warn!("{str}");
        Ok(())
    })?;
    let log_trace = lua.create_function(|_, str: String| {
        trace!("{str}");
        Ok(())
    })?;

    tbl.set("info", log_info)?;
    tbl.set("error", log_error)?;
    tbl.set("debug", log_debug)?;
    tbl.set("warn", log_warn)?;
    tbl.set("trace", log_trace)?;

    Ok(tbl)
}
