use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::{
    PkgCommand,
    init_logging,
    lua::{
        eval_config,
        plugin::{LuaPluginSpec, RemotePluginSpec},
    },
    paths::Paths,
    pkg::{git::Git, lockfile::LockfileEntry},
};

mod git;
mod lockfile;
mod manifest;
pub use lockfile::Lockfile;
pub use manifest::Manifest;

pub async fn run_pkg(command: PkgCommand) -> Result<()> {
    let _log_guards = init_logging("info")?;
    let (_, _, plugins_spec) = eval_config(None).await?;

    match command {
        PkgCommand::Upgrade => {
            let mut lockfile = Lockfile::read_or_default().await?;
            let mut specs = HashMap::new();

            for spec in plugins_spec.read().await.iter() {
                match &*spec.read().await {
                    LuaPluginSpec::Builtin(_) => {}
                    LuaPluginSpec::Local(_) => {}
                    LuaPluginSpec::Remote(spec) => {
                        specs.insert(normalize_url(&spec.url).to_string(), spec.clone());
                    }
                }
            }

            run_upgrade(specs.into_values().collect(), &mut lockfile).await?;
            lockfile.write().await?;
            Ok(())
        }
    }
}

pub struct AddResult {
    pub manifest: Manifest,
    pub plugin_dir: PathBuf,
}

pub async fn run_add(spec: &RemotePluginSpec, lockfile: &mut Lockfile) -> Result<AddResult> {
    run_inner(spec.clone(), lockfile, Resolution::Locked).await
}

pub async fn run_upgrade(specs: Vec<RemotePluginSpec>, lockfile: &mut Lockfile) -> Result<()> {
    let mut found_plugins = HashSet::new();

    for spec in specs {
        let url = spec.url.clone();
        tracing::info!(url, "Updating plugin");

        let plugin_name = url_to_dir_name(&spec.url);
        found_plugins.insert(plugin_name);

        if let Err(err) = run_inner(spec, lockfile, Resolution::Latest).await {
            tracing::error!(url, "Failed to update plugin");
            tracing::error!("{err}");
        }
    }

    lockfile.retain(|name, _| found_plugins.contains(name));

    Ok(())
}

pub async fn run_inner(
    spec: RemotePluginSpec,
    lockfile: &mut Lockfile,
    resolution: Resolution,
) -> Result<AddResult> {
    let plugin_name = url_to_dir_name(&spec.url);
    let target_dir = Paths::plugins_dir().join(&plugin_name);

    let freshly_cloned = ensure_cloned(&target_dir, &spec).await?;

    let result = resolve_and_read(&target_dir, &plugin_name, spec, lockfile, resolution).await;

    if result.is_err() && freshly_cloned {
        match target_dir.canonicalize() {
            Ok(canonicalized) => {
                if !canonicalized.starts_with(Paths::plugins_dir()) {
                    tracing::error!(
                        "Refusing to remove cloned plugin directory '{}', it is not a subdirectory of the plugins directory",
                        canonicalized.display()
                    );
                } else if let Err(err) = tokio::fs::remove_dir_all(&canonicalized).await {
                    tracing::error!(
                        "Failed to remove cloned plugin directory '{}': {err}",
                        canonicalized.display()
                    );
                }
            }
            Err(err) => {
                tracing::error!(
                    "Failed to canonicalize cloned plugin directory '{}': {err}",
                    target_dir.display()
                );
            }
        }
    }

    result
}

/// Makes sure `target_dir` contains a git repository for `spec`, cloning it if
/// necessary. Returns whether a fresh clone was performed.
async fn ensure_cloned(target_dir: &Path, spec: &RemotePluginSpec) -> Result<bool> {
    let is_repo = tokio::fs::try_exists(target_dir.join(".git")).await.with_context(|| {
        format!("Failed to check whether '{}' is a git repository", target_dir.display())
    })?;

    if is_repo {
        return Ok(false);
    }

    let dir_exists = tokio::fs::try_exists(target_dir).await.with_context(|| {
        format!("Failed to check whether plugin directory exists: '{}'", target_dir.display())
    })?;

    if dir_exists {
        tracing::warn!(
            "Plugin directory '{}' is not a git repository, removing it before cloning",
            target_dir.display()
        );

        match target_dir.canonicalize() {
            Ok(canonicalized) => {
                if canonicalized.starts_with(Paths::plugins_dir()) {
                    tokio::fs::remove_dir_all(canonicalized).await.with_context(|| {
                        format!(
                            "Failed to remove stale plugin directory: '{}'",
                            target_dir.display()
                        )
                    })?;
                } else {
                    bail!(
                        "Refusing to remove cloned plugin directory '{}', it is not a subdirectory of the plugins directory",
                        canonicalized.display()
                    );
                }
            }
            Err(err) => {
                bail!(
                    "Failed to canonicalize cloned plugin directory '{}': {err}",
                    target_dir.display()
                );
            }
        }
    }

    tracing::debug!(url = spec.url, "Cloning plugin");

    Git::clone(&spec.url, target_dir, spec.branch.as_deref())
        .await
        .context("Failed to clone plugin")?;

    Ok(true)
}

async fn resolve_and_read(
    target_dir: &Path,
    plugin_name: &str,
    spec: RemotePluginSpec,
    lockfile: &mut Lockfile,
    resolution: Resolution,
) -> Result<AddResult> {
    let current_rev =
        Git::rev(target_dir, "HEAD").await.context("Failed to get plugin's git rev")?;

    let locked_rev = match resolution {
        Resolution::Latest => None,
        Resolution::Locked => lockfile
            .get(plugin_name)
            .and_then(|entry| locked_rev_for_spec(entry, &spec, plugin_name)),
    };

    let target_rev =
        get_target_rev(target_dir, spec.branch.as_deref(), &current_rev, locked_rev.as_deref())
            .await
            .context("Failed to get target revision")?;

    if current_rev == target_rev {
        tracing::debug!(plugin_name, "Plugin is already at up to date");
    } else {
        Git::checkout(target_dir, &target_rev)
            .await
            .context("Failed to checkout plugin revision")?;
    }

    let manifest = Manifest::read(target_dir)
        .await
        .inspect_err(|err| tracing::error!("Failed to read plugin manifest: {err}"))?;

    tracing::info!("Successfully added plugin: {}", manifest.name);
    lockfile.record(plugin_name.to_owned(), spec.branch, spec.url, target_rev);

    Ok(AddResult { manifest, plugin_dir: target_dir.to_path_buf() })
}

async fn get_target_rev(
    repo_dir: &Path,
    branch: Option<&str>,
    current_rev: &str,
    locked_rev: Option<&str>,
) -> Result<String> {
    match locked_rev {
        Some(locked_rev) if current_rev == locked_rev => Ok(current_rev.to_owned()),
        Some(locked_rev) => {
            Git::fetch(repo_dir, None).await.context("Failed to fetch plugin updates")?;
            Ok(locked_rev.to_owned())
        }
        None => {
            Git::fetch(repo_dir, None).await.context("Failed to fetch plugin updates")?;
            let branch = match branch {
                Some(branch) => branch.to_string(),
                None => {
                    Git::default_branch(repo_dir).await.context("Failed to get default branch")?
                }
            };

            let target = format!("origin/{branch}");
            Git::rev(repo_dir, &target)
                .await
                .with_context(|| format!("Failed to resolve branch '{branch}'"))
        }
    }
}

fn normalize_url(url: &str) -> &str {
    let normalized = url.trim().trim_end_matches('/');
    let normalized = normalized.strip_suffix(".git").unwrap_or(normalized);
    normalized.trim_end_matches('/')
}

fn url_to_dir_name(url: &str) -> String {
    let normalized = normalize_url(url);

    let plugin_name = normalized
        .rsplit('/')
        .next()
        .unwrap_or(normalized)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect::<String>();

    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    format!("{plugin_name}-{}", &hash[..16])
}

fn locked_rev_for_spec(
    entry: &LockfileEntry,
    spec: &RemotePluginSpec,
    plugin_name: &str,
) -> Option<String> {
    if normalize_url(&entry.url) != normalize_url(&spec.url) {
        tracing::info!(
            plugin = plugin_name,
            locked = entry.url,
            configured = spec.url,
            "Plugin url changed since it was locked, re-resolving"
        );
        return None;
    }

    if entry.branch.as_deref() != spec.branch.as_deref() {
        tracing::info!(
            plugin = plugin_name,
            locked = ?entry.branch,
            configured = ?spec.branch,
            "Plugin branch changed since it was locked, re-resolving"
        );
        return None;
    }

    Some(entry.rev.clone())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    Locked,
    Latest,
}
