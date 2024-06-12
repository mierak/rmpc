use std::path::{Path, PathBuf};

use anyhow::Context;
use anyhow::Result;
use clap::{Parser, Subcommand};
use log::Level;
use serde::{Deserialize, Serialize};

mod defaults;
mod keys;
pub mod theme;

use self::{
    keys::{KeyConfig, KeyConfigFile},
    theme::{ConfigColor, UiConfig, UiConfigFile},
};

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long, value_name = "FILE", default_value = get_default_config_path().into_os_string())]
    pub config: PathBuf,
    #[arg(short, long, default_value_t = Level::Debug)]
    pub log: Level,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Clone, Debug, PartialEq)]
pub enum Command {
    /// Prints the default config. Can be used to bootstrap your config file.
    Config,
    /// Prints the default theme. Can be used to bootstrap your theme file.
    Theme,
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
    path.push(env!("CARGO_CRATE_NAME"));
    path.push("config.ron");
    return path;
}

#[derive(Debug, Default)]
pub struct Config {
    pub address: &'static str,
    pub volume_step: u8,
    pub keybinds: KeyConfig,
    pub status_update_interval_ms: Option<u64>,
    pub theme: UiConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFile {
    address: String,
    #[serde(default)]
    theme: Option<String>,
    #[serde(default = "defaults::default_volume_step")]
    volume_step: u8,
    #[serde(default = "defaults::default_progress_update_interval_ms")]
    status_update_interval_ms: Option<u64>,
    #[serde(default)]
    keybinds: KeyConfigFile,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            address: String::from("127.0.0.1:6600"),
            keybinds: KeyConfigFile::default(),
            volume_step: 5,
            status_update_interval_ms: Some(1000),
            theme: None,
        }
    }
}

impl ConfigFile {
    pub fn read(path: &PathBuf) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let read = std::io::BufReader::new(file);
        Ok(ron::de::from_reader(read)?)
    }

    fn read_theme(&self, config_dir: &Path) -> Result<UiConfig> {
        self.theme.as_ref().map_or_else(
            || UiConfigFile::default().try_into(),
            |theme_name| -> Result<_> {
                let path = PathBuf::from(config_dir)
                    .join("themes")
                    .join(format!("{theme_name}.ron"));
                let file = std::fs::File::open(&path)
                    .with_context(|| format!("Failed to open theme file {:?}", path.to_string_lossy()))?;
                let read = std::io::BufReader::new(file);
                let theme: UiConfigFile = ron::de::from_reader(read)?;
                theme.try_into()
            },
        )
    }

    pub fn into_config(self, config_dir: &Path) -> Result<Config> {
        Ok(Config {
            theme: self.read_theme(config_dir.parent().expect("Config path to be defined correctly"))?,
            address: Box::leak(Box::new(self.address)),
            volume_step: self.volume_step,
            status_update_interval_ms: self.status_update_interval_ms.map(|v| v.max(100)),
            keybinds: self.keybinds.into(),
        })
    }
}
