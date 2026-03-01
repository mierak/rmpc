use anyhow::{Result, bail};
use mlua::{Lua, Table};
use rmpc_shared::paths::rmpcd_config_dir;
use tracing::{debug, error, info, trace, warn};

pub fn init() -> Result<(Lua, Table)> {
    let Some(config_dir) = rmpcd_config_dir() else {
        bail!("Could not determine config directory");
    };
    let rmpcd_pkg_path =
        format!("{}/?.lua;{}/?/init.lua", config_dir.display(), config_dir.display());

    let lua = Lua::new();
    let globals = lua.globals();
    let package: Table = globals.get("package")?;
    let package_path = package.get::<String>("path")?;

    package.set("path", format!("{rmpcd_pkg_path};{package_path}"))?;

    create_logger(&lua)?;

    let file = std::fs::read(config_dir.join("init.lua"))?;
    let lua_config: Table = lua.load(&file).eval()?;

    Ok((lua, lua_config))
}

fn create_logger(lua: &Lua) -> Result<()> {
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
    let logger = lua.create_table()?;
    logger.set("info", log_info)?;
    logger.set("error", log_error)?;
    logger.set("debug", log_debug)?;
    logger.set("warn", log_warn)?;
    logger.set("trace", log_trace)?;
    lua.globals().set("log", logger)?;

    Ok(())
}
