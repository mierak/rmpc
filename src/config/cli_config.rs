use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{Config, ConfigFile, MpdAddress, address::MpdPassword, utils::tilde_expand};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CliConfigFile {
    #[serde(default = "super::defaults::mpd_address")]
    pub address: String,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    cache_dir: Option<PathBuf>,
    #[serde(default)]
    lyrics_dir: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct CliConfig {
    pub address: MpdAddress,
    pub password: Option<MpdPassword>,
    pub cache_dir: Option<PathBuf>,
    pub lyrics_dir: Option<String>,
}

impl From<ConfigFile> for CliConfigFile {
    fn from(value: ConfigFile) -> Self {
        Self {
            address: value.address,
            password: value.password,
            cache_dir: value.cache_dir,
            lyrics_dir: value.lyrics_dir,
        }
    }
}

impl From<Config> for CliConfig {
    fn from(value: Config) -> Self {
        Self {
            address: value.address,
            password: value.password,
            cache_dir: value.cache_dir,
            lyrics_dir: value.lyrics_dir,
        }
    }
}

impl From<&Config> for CliConfig {
    fn from(value: &Config) -> Self {
        Self {
            address: value.address.clone(),
            password: value.password.clone(),
            cache_dir: value.cache_dir.clone(),
            lyrics_dir: value.lyrics_dir.clone(),
        }
    }
}

impl CliConfigFile {
    pub fn read(path: &PathBuf) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let read = std::io::BufReader::new(file);
        let config: CliConfigFile = ron::de::from_reader(read)?;

        Ok(config)
    }

    pub fn into_config(
        self,
        address_cli: Option<String>,
        password_cli: Option<String>,
    ) -> CliConfig {
        let (address, password) =
            MpdAddress::resolve(address_cli, password_cli, self.address, self.password);

        CliConfig {
            cache_dir: self.cache_dir,
            lyrics_dir: self.lyrics_dir.map(|v| {
                let v = tilde_expand(&v);
                if v.ends_with('/') { v.into_owned() } else { format!("{v}/") }
            }),
            address,
            password,
        }
    }
}
