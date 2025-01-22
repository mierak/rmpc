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
    cache_dir: Option<String>,
    #[serde(default)]
    lyrics_dir: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct CliConfig {
    pub address: MpdAddress<'static>,
    pub password: Option<MpdPassword<'static>>,
    pub cache_dir: Option<&'static str>,
    pub lyrics_dir: Option<&'static str>,
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

impl<'a> From<&'a Config> for CliConfig {
    fn from(value: &'a Config) -> Self {
        Self {
            address: value.address,
            password: value.password,
            cache_dir: value.cache_dir,
            lyrics_dir: value.lyrics_dir,
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
            cache_dir: self
                .cache_dir
                .map(|v| if v.ends_with('/') { v } else { format!("{v}/") }.leak() as &'static _),
            lyrics_dir: self.lyrics_dir.map(|v| {
                let v = tilde_expand(&v);
                if v.ends_with('/') { v.into_owned() } else { format!("{v}/") }.leak() as &'static _
            }),
            address,
            password,
        }
    }
}
