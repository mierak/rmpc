use std::{path::PathBuf, sync::Arc};

use mlua::{ExternalResult, Lua};
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub enum LuaPluginSpec {
    Builtin(BuiltinPluginSpec),
    Local(LocalPluginSpec),
    Remote(RemotePluginSpec),
}

impl LuaPluginSpec {
    pub fn args_mut(&mut self) -> &mut String {
        match self {
            LuaPluginSpec::Builtin(spec) => &mut spec.args,
            LuaPluginSpec::Local(spec) => &mut spec.args,
            LuaPluginSpec::Remote(spec) => &mut spec.args,
        }
    }

    pub async fn determine(
        lua: &Lua,
        args: mlua::Value,
        plugins_specs: &Arc<RwLock<Vec<Arc<RwLock<LuaPluginSpec>>>>>,
    ) -> Result<mlua::Table, mlua::Error> {
        let record_entry = async |entry: LuaPluginSpec| -> mlua::Result<mlua::Table> {
            let entry = Arc::new(RwLock::new(entry));
            let entry_clone = entry.clone();
            plugins_specs.write().await.push(entry);

            let tbl = lua.create_table()?;
            let setup = lua.create_async_function(
                move |_lua, (_self, args): (mlua::Value, mlua::Value)| {
                    let entry_clone = entry_clone.clone();
                    async move {
                        let json = serde_json::to_string(&args).into_lua_err()?;
                        *entry_clone.write().await.args_mut() = json;
                        Ok(())
                    }
                },
            )?;

            tbl.raw_set("setup", setup)?;

            Ok(tbl)
        };

        let tbl = match args {
            mlua::Value::String(args) => {
                let str = args.to_str()?;

                let entry = if let Some(name) = str.strip_prefix("#builtin.") {
                    LuaPluginSpec::Builtin(BuiltinPluginSpec::new(
                        name.to_string(),
                        String::from("{}"),
                    ))
                } else {
                    let mut path = PathBuf::new();
                    if str.is_empty() || str == "." {
                        return Err(mlua::Error::external("Plugin name cannot be empty"));
                    }

                    let split = str.split('.');

                    for segment in split {
                        path.push(segment);
                    }
                    LuaPluginSpec::Local(LocalPluginSpec::new(path, String::from("{}")))
                };

                record_entry(entry).await?
            }
            mlua::Value::Table(table) => {
                let git_url = table.get::<String>("url")?;
                let branch = table.get::<Option<String>>("branch")?;

                if git_url.is_empty() {
                    return Err(mlua::Error::external("Plugin url cannot be empty"));
                }

                let entry = LuaPluginSpec::Remote(RemotePluginSpec::new(
                    git_url,
                    branch,
                    String::from("{}"),
                ));

                record_entry(entry).await?
            }
            arg => {
                return Err(anyhow::anyhow!(
                    "Invalid argument type for install, expected string or table: {arg:?}"
                ))
                .into_lua_err();
            }
        };

        Ok(tbl)
    }
}

#[derive(Clone, Debug)]
pub struct LocalPluginSpec {
    pub path: PathBuf,
    pub args: String,
}

impl LocalPluginSpec {
    pub fn new(path: PathBuf, args: String) -> Self {
        Self { path, args }
    }
}

#[derive(Clone, Debug)]
pub struct BuiltinPluginSpec {
    pub name: String,
    pub args: String,
}

impl BuiltinPluginSpec {
    pub fn new(name: String, args: String) -> Self {
        Self { name, args }
    }
}

#[derive(Clone, Debug)]
pub struct RemotePluginSpec {
    pub url: String,
    pub branch: Option<String>,
    pub args: String,
}

impl RemotePluginSpec {
    pub fn new(url: String, branch: Option<String>, args: String) -> Self {
        Self { url, branch, args }
    }
}
