use std::{path::Path, sync::Arc};

use anyhow::Result;
use mlua::{Lua, Table};
use tokio::sync::RwLock;

use crate::{
    async_client::AsyncClient,
    lua::plugin::{LuaPluginEntry, PluginEvent},
};

pub mod lualib;
pub mod plugin;
pub mod type_def_eject;

pub fn create(
    cfg_dir: &Path,
    client: &Arc<AsyncClient>,
    plugins: Option<&Arc<RwLock<Vec<Arc<RwLock<LuaPluginEntry>>>>>>,
) -> Result<Lua> {
    let lua = Lua::new();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<PluginEvent>();
    {
        let tx = tx.clone();
        lua.set_app_data(tx);
    }

    let rmpcd_pkg_path = format!("{}/?.lua;{}/?/init.lua", cfg_dir.display(), cfg_dir.display());

    let package: Table = lua.globals().get("package")?;
    let package_path = package.get::<String>("path")?;

    package.set("path", format!("{rmpcd_pkg_path};{package_path}"))?;

    let rmpcd = lua.create_table()?;
    lua.globals().raw_set("rmpcd", &rmpcd)?;

    install_lib(&lua, client, plugins)?;
    install_builtins(&lua)?;

    Ok(lua)
}

pub async fn eval_config(lua: &Lua, cfg_dir: &Path) -> Result<Table> {
    let file = std::fs::read(cfg_dir.join("init.lua"))?;
    let lua_config: Table = lua.load(&file).eval_async().await?;

    Ok(lua_config)
}

pub fn install_lib(
    lua: &Lua,
    client: &Arc<AsyncClient>,
    plugins: Option<&Arc<RwLock<Vec<Arc<RwLock<LuaPluginEntry>>>>>>,
) -> mlua::Result<()> {
    macro_rules! install_lib {
        ($name:ident) => {
            let lib = crate::lua::lualib::$name::create(lua)?;
            lua.globals().raw_set(stringify!($name), lib)?;
        };
    }

    lualib::plugin::init(lua, plugins)?;

    let mpd = lualib::mpd::create(lua, client)?;
    lua.globals().raw_set("mpd", mpd)?;

    install_lib!(log);
    install_lib!(sync);
    install_lib!(process);
    install_lib!(http);
    install_lib!(fs);
    install_lib!(util);

    Ok(())
}

pub fn install_builtins(lua: &Lua) -> mlua::Result<()> {
    macro_rules! install_builtin {
        ($name:literal) => {
            lua.load(include_str!(concat!("./builtin/", $name, ".lua")))
                .set_name(concat!("#builtin/", $name, ".lua"))
                .call::<Table>(())?;
        };
    }

    install_builtin!("notify");
    install_builtin!("playcount");
    install_builtin!("lyrics");
    install_builtin!("lastfm");

    Ok(())
}
