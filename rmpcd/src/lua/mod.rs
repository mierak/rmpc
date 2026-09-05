use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use mlua::{Lua, Table};
use tokio::sync::RwLock;

use crate::{
    async_client::AsyncClient,
    lua::plugin::{LuaPluginSpec, PluginEvent},
    paths::Paths,
};

pub mod lualib;
pub mod plugin;
pub mod type_def_eject;

pub fn create(
    additional_package_path: Option<&Path>,
    client: &Arc<AsyncClient>,
    plugins: Option<&Arc<RwLock<Vec<Arc<RwLock<LuaPluginSpec>>>>>>,
) -> Result<Lua> {
    let lua = Lua::new();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<PluginEvent>();
    {
        let tx = tx.clone();
        lua.set_app_data(tx);
    }

    if let Some(additional_package_path) = additional_package_path {
        let rmpcd_pkg_path = format!(
            "{}/?.lua;{}/?/init.lua",
            additional_package_path.display(),
            additional_package_path.display()
        );

        let package: Table = lua.globals().get("package")?;
        let package_path = package.get::<String>("path")?;

        package.set("path", format!("{rmpcd_pkg_path};{package_path}"))?;
    }

    let rmpcd = lua.create_table()?;
    lua.globals().raw_set("rmpcd", &rmpcd)?;

    install_lib(&lua, client, plugins)?;
    install_builtins(&lua)?;

    Ok(lua)
}

pub async fn eval_config(
    mpd: Option<Arc<AsyncClient>>,
) -> Result<(Lua, mlua::Table, Arc<RwLock<Vec<Arc<RwLock<LuaPluginSpec>>>>>)> {
    let mpd = mpd.unwrap_or(Arc::new(AsyncClient::new(|_| {}, || {})));

    let cfg_dir = Paths::config_dir();

    let plugins: Arc<RwLock<Vec<_>>> = Arc::new(RwLock::new(Vec::new()));
    let lua = create(Some(cfg_dir), &mpd, Some(&plugins))?;

    let file = std::fs::read(cfg_dir.join("init.lua"))
        .context("Failed to read config. Did you initialize your config? Try 'rmpcd init'")?;
    let lua_config: Table = lua.load(&file).eval_async().await?;

    Ok((lua, lua_config, plugins))
}

pub fn install_lib(
    lua: &Lua,
    client: &Arc<AsyncClient>,
    plugins: Option<&Arc<RwLock<Vec<Arc<RwLock<LuaPluginSpec>>>>>>,
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
