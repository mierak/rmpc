use std::{path::Path, sync::Arc};

use anyhow::Result;
use mlua::{Lua, Table};
use tokio::sync::RwLock;

use crate::{async_client::AsyncClient, lua::plugin::LuaPluginEntry};

pub mod lualib;
pub mod plugin;

pub fn create(
    cfg_dir: &Path,
    client: &Arc<AsyncClient>,
    plugins: Option<&Arc<RwLock<Vec<Arc<RwLock<LuaPluginEntry>>>>>>,
) -> Result<Lua> {
    let lua = Lua::new();

    let rmpcd_pkg_path = format!("{}/?.lua;{}/?/init.lua", cfg_dir.display(), cfg_dir.display());

    let package: Table = lua.globals().get("package")?;
    let package_path = package.get::<String>("path")?;
    let preload = package.get::<Table>("preload")?;

    package.set("path", format!("{rmpcd_pkg_path};{package_path}"))?;

    let rmpcd = lua.create_table()?;
    lua.globals().raw_set("rmpcd", &rmpcd)?;

    install_lib(&lua, &preload, client, plugins)?;
    install_builtins(&lua, &preload)?;

    Ok(lua)
}

pub async fn eval_config(lua: &Lua, cfg_dir: &Path) -> Result<Table> {
    let file = std::fs::read(cfg_dir.join("init.lua"))?;
    let lua_config: Table = lua.load(&file).eval_async().await?;

    Ok(lua_config)
}

pub fn install_lib(
    lua: &Lua,
    preload: &Table,
    client: &Arc<AsyncClient>,
    plugins: Option<&Arc<RwLock<Vec<Arc<RwLock<LuaPluginEntry>>>>>>,
) -> mlua::Result<()> {
    macro_rules! install_lib {
        ($name:ident) => {
            let lib = lualib::$name::create(lua)?;
            preload.raw_set(
                concat!("rmpcd.", stringify!($name)),
                lua.create_function(move |_, ()| Ok(lib.clone()))?,
            )?;
        };
    }
    lualib::hooks::init(lua, plugins)?;

    let mpd = lualib::mpd::create(lua, client)?;
    preload.raw_set("rmpcd.mpd", lua.create_function(move |_, ()| Ok(mpd.clone()))?)?;

    install_lib!(log);
    install_lib!(sync);
    install_lib!(process);
    install_lib!(http);
    install_lib!(fs);
    install_lib!(util);

    Ok(())
}

pub fn install_builtins(lua: &Lua, preload: &Table) -> mlua::Result<()> {
    macro_rules! install_builtin {
        ($name:literal) => {
            let tbl = lua
                .load(include_str!(concat!("./builtin/", $name, ".lua")))
                .set_name($name)
                .call::<Table>(())?;
            preload.set(
                concat!("rmpcd.", $name),
                lua.create_function(move |_, ()| Ok(tbl.clone()))?,
            )?;
        };
    }

    // Sync modifies the preload table directly
    lua.load(include_str!("./builtin/sync.lua")).set_name("sync").exec()?;

    install_builtin!("notify");
    install_builtin!("playcount");
    install_builtin!("lyrics");
    install_builtin!("lastfm");

    Ok(())
}
