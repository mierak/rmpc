use anyhow::Result;
use mlua::Lua;
use tracing::{debug, error, info, trace, warn};

pub fn init(lua: &Lua) -> Result<()> {
    let logger = lua.create_table()?;

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

    logger.set("info", log_info)?;
    logger.set("error", log_error)?;
    logger.set("debug", log_debug)?;
    logger.set("warn", log_warn)?;
    logger.set("trace", log_trace)?;
    lua.globals().raw_set("log", logger)?;

    Ok(())
}
