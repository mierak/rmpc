use std::path::PathBuf;

use rmpc_mpd::address::MpdPassword;
use rmpc_shared::paths::utils::{absolute_env_var_expand_path, env_var_expand, tilde_expand};
use serde::{Deserialize, Serialize};

use super::{Config, ConfigFile, MpdAddress};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CliConfigFile {
    pub address: String,
    password: Option<String>,
    cache_dir: Option<PathBuf>,
    lyrics_dir: Option<String>,
    extra_yt_dlp_args: Vec<String>,
}

impl Default for CliConfigFile {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:6600".to_string(),
            password: None,
            cache_dir: None,
            lyrics_dir: None,
            extra_yt_dlp_args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CliConfig {
    pub address: MpdAddress,
    pub password: Option<MpdPassword>,
    pub cache_dir: Option<PathBuf>,
    pub lyrics_dir: Option<String>,
    pub extra_yt_dlp_args: Vec<String>,
}

impl Default for CliConfig {
    fn default() -> Self {
        CliConfigFile::default().into_config(None, None)
    }
}

impl From<ConfigFile> for CliConfigFile {
    fn from(value: ConfigFile) -> Self {
        Self {
            address: value.address,
            password: value.password,
            cache_dir: value.cache_dir,
            lyrics_dir: value.lyrics_dir,
            extra_yt_dlp_args: value.extra_yt_dlp_args,
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
            extra_yt_dlp_args: value.extra_yt_dlp_args,
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
            extra_yt_dlp_args: value.extra_yt_dlp_args.clone(),
        }
    }
}

impl CliConfigFile {
    pub fn into_config(
        self,
        address_cli: Option<String>,
        password_cli: Option<String>,
    ) -> CliConfig {
        let (address, password) =
            rmpc_mpd::address::resolve(address_cli, password_cli, self.address, self.password);

        CliConfig {
            cache_dir: self
                .cache_dir
                .map(|v| -> Option<PathBuf> {
                    absolute_env_var_expand_path(&v).unwrap_or_else(|err| {
                        log::warn!("Failed to expand cache_dir path '{}': {err}", v.display());
                        None
                    })
                })
                .unwrap_or_default(),
            lyrics_dir: self.lyrics_dir.map(|v| {
                let v = env_var_expand(&v);
                let v = tilde_expand(&v);
                if v.ends_with('/') { v.into_owned() } else { format!("{v}/") }
            }),
            address,
            password,
            extra_yt_dlp_args: self.extra_yt_dlp_args,
        }
    }
}
