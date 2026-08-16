use std::{collections::HashSet, path::Path, sync::Arc};

use anyhow::{Context, Result, bail};
use mlua::{LuaSerdeExt, Table};
use tokio::sync::RwLock;

use crate::{
    async_client::AsyncClient,
    lua::{
        self,
        lualib::plugin::{ON_IDLE, ON_MESSAGE, ON_SHUTDOWN, ON_SONG_CHANGE, ON_STATE_CHANGE},
        plugin::{
            LuaPlugin,
            LuaPluginSpec,
            RemotePluginSpec,
            spec::{BuiltinPluginSpec, LocalPluginSpec},
            triggers::Triggers,
        },
    },
    pkg::{AddResult, Lockfile, run_add},
};

const LASTFM: &str = include_str!("../builtin/lastfm.lua");
const NOTIFY: &str = include_str!("../builtin/notify.lua");
const PLAYCOUNT: &str = include_str!("../builtin/playcount.lua");
const LYRICS: &str = include_str!("../builtin/lyrics.lua");

pub async fn load(
    cfg_dir: &Path,
    plugin: &Arc<RwLock<LuaPluginSpec>>,
    client: &Arc<AsyncClient>,
    lockfile: &mut Lockfile,
) -> Result<LuaPlugin> {
    match &*plugin.read().await {
        LuaPluginSpec::Builtin(spec) => load_builtin(spec, client).await,
        LuaPluginSpec::Local(spec) => load_local(spec, cfg_dir, client).await,
        LuaPluginSpec::Remote(spec) => load_remote(spec, client, lockfile).await,
    }
}

async fn load_builtin(spec: &BuiltinPluginSpec, client: &Arc<AsyncClient>) -> Result<LuaPlugin> {
    let content = match spec.name.as_str() {
        "lastfm" => LASTFM,
        "notify" => NOTIFY,
        "playcount" => PLAYCOUNT,
        "lyrics" => LYRICS,
        _ => bail!("Unknown builtin plugin: {}", spec.name),
    };
    let name = format!("#builtin/{}", spec.name);

    load_single(content, name, &spec.args, None, client).await
}

async fn load_remote(
    spec: &RemotePluginSpec,
    client: &Arc<AsyncClient>,
    lockfile: &mut Lockfile,
) -> Result<LuaPlugin> {
    let AddResult { manifest, plugin_dir } = run_add(spec, lockfile).await?;

    let name = format!("#pkg/{}/{}", manifest.author, manifest.name);

    let canonicalized_plugin_dir = plugin_dir.canonicalize().with_context(|| {
        format!("Invalid or missing plugin directory: {}", plugin_dir.display())
    })?;
    let entry_path = plugin_dir.join(&manifest.entry_point).canonicalize().with_context(|| {
        format!("Invalid or missing plugin entry point: {}", manifest.entry_point.display())
    })?;

    if !entry_path.starts_with(canonicalized_plugin_dir) {
        bail!(
            "Plugin entry point {} is outside of plugin directory {}",
            entry_path.display(),
            plugin_dir.display()
        );
    }

    let content = tokio::fs::read_to_string(&entry_path).await?;

    load_single(content, name, &spec.args, Some(&plugin_dir), client).await
}

async fn load_local(
    spec: &LocalPluginSpec,
    cfg_dir: &Path,
    client: &Arc<AsyncClient>,
) -> Result<LuaPlugin> {
    let mut plugin_path = spec.path.clone();
    plugin_path.set_extension("lua");
    let plugin_path = cfg_dir.join(plugin_path);
    let content = tokio::fs::read(&plugin_path)
        .await
        .with_context(|| format!("Invalid or missing plugin path: {}", plugin_path.display()))?;

    load_single(
        content,
        spec.path.to_string_lossy().into_owned(),
        &spec.args,
        Some(cfg_dir),
        client,
    )
    .await
}

async fn load_single(
    content: impl AsRef<[u8]>,
    name: String,
    args: &str,
    additional_pkg_path: Option<&Path>,
    client: &Arc<AsyncClient>,
) -> Result<LuaPlugin> {
    let lua = lua::create(additional_pkg_path, client, None)?;
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    {
        let tx = tx.clone();
        lua.set_app_data(tx);
    }

    let state: Table = lua.load(content.as_ref()).set_name(&name).eval_async().await?;

    let song_change = state.contains_key(ON_SONG_CHANGE)?;
    let state_change = state.contains_key(ON_STATE_CHANGE)?;
    let message = state.contains_key(ON_MESSAGE)?;
    let idle = state.contains_key(ON_IDLE)?;
    let shutdown = state.contains_key(ON_SHUTDOWN)?;
    let mut triggers = Triggers::empty();
    if song_change {
        triggers |= Triggers::SongChange;
    }
    if state_change {
        triggers |= Triggers::StateChange;
    }
    if message {
        triggers |= Triggers::Message;
    }
    if idle {
        triggers |= Triggers::Idle;
    }
    if shutdown {
        triggers |= Triggers::Shutdown;
    }

    if triggers.is_empty() {
        bail!("Plugin must have at least one trigger");
    }

    if let Some(setup) = state.get::<Option<mlua::Function>>("setup")? {
        let args = lua.to_value(&serde_json::from_str::<serde_json::Value>(args)?)?;
        setup.call_async::<()>((&state, args)).await?;
    }

    let subscribed_channels = state.get::<Option<Vec<String>>>("subscribed_channels")?;
    let subscribed_channels = if let Some(channels) = subscribed_channels {
        channels.into_iter().collect()
    } else {
        HashSet::new()
    };

    Ok(LuaPlugin::new(name, triggers, subscribed_channels, tx, lua, state, rx))
}
