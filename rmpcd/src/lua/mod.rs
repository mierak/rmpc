use anyhow::{Result, bail};
use mlua::{Lua, Table};
use rmpc_shared::paths::rmpcd_config_dir;

mod lualib;

pub fn init() -> Result<(Lua, Table)> {
    let Some(config_dir) = rmpcd_config_dir() else {
        bail!("Could not determine config directory");
    };
    let rmpcd_pkg_path =
        format!("{}/?.lua;{}/?/init.lua", config_dir.display(), config_dir.display());

    let lua = Lua::new();
    let package: Table = lua.globals().get("package")?;
    let package_path = package.get::<String>("path")?;

    package.set("path", format!("{rmpcd_pkg_path};{package_path}"))?;

    let rmpcd = lua.create_table()?;
    lua.globals().raw_set("rmpcd", &rmpcd)?;

    lualib::log::init(&lua)?;
    lualib::sync::init(&lua)?;
    lualib::process::init(&lua)?;
    lualib::hooks::init(&lua)?;

    install_builtins(&lua)?;

    let file = std::fs::read(config_dir.join("init.lua"))?;
    let lua_config: Table = lua.load(&file).eval()?;

    Ok((lua, lua_config))
}

fn install_builtins(lua: &Lua) -> mlua::Result<()> {
    lua.load(include_str!("./builtin/notify.lua")).set_name("notify").exec()?;
    lua.load(include_str!("./builtin/sync.lua")).set_name("sync").exec()?;

    Ok(())
}
