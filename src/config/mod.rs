use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tracing::Level;

mod defaults;
mod keys;
mod ui;
pub use self::ui::{SongProperty, SymbolsConfig};

use self::{
    keys::{KeyConfig, KeyConfigFile},
    ui::{ConfigColor, UiConfig, UiConfigFile},
};

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long, value_name = "FILE", default_value = get_default_config_path().into_os_string())]
    pub config: PathBuf,
    #[arg(short, long, default_value_t = Level::DEBUG)]
    pub log: Level,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Clone, Debug, PartialEq)]
pub enum Command {
    /// Prints the default config. Can be used to bootstrap your config file.
    Config,
}

fn get_default_config_path() -> PathBuf {
    let mut path = PathBuf::new();
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        path.push(dir);
    } else if let Ok(home) = std::env::var("HOME") {
        path.push(home);
        path.push(".config");
    } else {
        return path;
    }
    path.push("mpdox");
    path.push("config.ron");
    return path;
}

#[derive(Debug)]
pub struct Config {
    pub address: &'static str,
    pub volume_step: u8,
    pub keybinds: KeyConfig,
    pub status_update_interval_ms: Option<u64>,
    pub ui: UiConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFile {
    address: String,
    #[serde(default = "defaults::default_volume_step")]
    volume_step: u8,
    #[serde(default = "defaults::default_progress_update_interval_ms")]
    status_update_interval_ms: Option<u64>,
    keybinds: KeyConfigFile,
    ui: Option<UiConfigFile>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            address: String::from("127.0.0.1:6600"),
            keybinds: KeyConfigFile::default(),
            volume_step: 5,
            status_update_interval_ms: Some(1000),
            ui: Some(UiConfigFile::default()),
        }
    }
}

impl TryFrom<ConfigFile> for Config {
    type Error = anyhow::Error;

    fn try_from(value: ConfigFile) -> Result<Self, Self::Error> {
        Ok(Self {
            ui: value.ui.unwrap_or_default().try_into()?,
            address: Box::leak(Box::new(value.address)),
            volume_step: value.volume_step,
            status_update_interval_ms: value.status_update_interval_ms.map(|v| v.max(100)),
            keybinds: value.keybinds.into(),
        })
    }
}
