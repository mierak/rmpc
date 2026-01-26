use std::path::{Path, PathBuf};

use crate::{config::utils::tilde_expand, shared::env::ENV};

#[cfg(debug_assertions)]
const CONFIG_NAME: &str = "config.debug.ron";
#[cfg(not(debug_assertions))]
const CONFIG_NAME: &str = "config.ron";
const CRATE_NAME: &str = env!("CARGO_CRATE_NAME");

pub fn home_dir() -> Option<PathBuf> {
    ENV.var_os("HOME")
        .and_then(|home| if home.is_empty() { None } else { Some(home) })
        .map(PathBuf::from)
}

pub fn config_dir() -> Option<PathBuf> {
    ENV.var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home_dir().map(|home| home.join(".config")))
        .map(|p| p.join(CRATE_NAME))
}

pub fn config_paths(cli_arg_config_path: Option<&Path>) -> Vec<PathBuf> {
    if let Some(path) = cli_arg_config_path {
        return vec![path.to_path_buf()];
    }

    let mut result = Vec::new();
    match config_dir() {
        Some(config_dir) => result.push(config_dir.join(CONFIG_NAME)),
        None => log::warn!("Could not determine configuration directory"),
    }

    if let Some(home) = home_dir() {
        result.push(home.join(CRATE_NAME).join(CONFIG_NAME));
    }

    result
}

pub fn theme_paths(
    cli_arg_theme: Option<&Path>,
    config_path: &Path,
    theme_name: &str,
) -> Vec<PathBuf> {
    if let Some(path) = cli_arg_theme {
        return vec![path.to_path_buf()];
    }

    let config_dir = config_path.parent().unwrap_or_else(|| {
        panic!("Expected config path to have parent directory. Path: '{}'", config_path.display())
    });

    vec![
        config_dir.join("themes").join(format!("{theme_name}.ron")),
        config_dir.join("themes").join(theme_name),
        config_dir.join(format!("{theme_name}.ron")),
        config_dir.join(theme_name),
        PathBuf::from(tilde_expand(theme_name).into_owned()),
    ]
}
