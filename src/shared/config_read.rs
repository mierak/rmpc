use std::{
    io::Read,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::{
    config::{
        Config,
        ConfigFile,
        cli::Args,
        cli_config::{CliConfig, CliConfigFile},
        theme::{UiConfig, UiConfigFile},
    },
    shared::paths::{config_paths, theme_paths},
};

#[derive(Error, Debug)]
pub enum ConfigReadError {
    #[error("Deserialization error, {0}")]
    Deserialization(#[from] serde_path_to_error::Error<ron::Error>),
    #[error("Failed to deserialize ron config file, {0}")]
    Ron(#[from] ron::error::SpannedError),
    #[error("Configuration file not found at any of the possible paths")]
    ConfigNotFound,
    #[error("Theme file not found at any of the possible paths")]
    ThemeNotFound,
    #[error("IO error, {0}")]
    Io(#[from] std::io::Error),
    #[error("No configuration paths available")]
    NoConfigPaths,
    #[error("{0:?}")]
    Conversion(#[from] anyhow::Error),
}

pub fn read_cli_config(
    cli_arg_config_path: Option<&Path>,
    cli_arg_address: Option<String>,
    cli_arg_password: Option<String>,
) -> Result<CliConfig, ConfigReadError> {
    let config_paths = config_paths(cli_arg_config_path);
    if config_paths.is_empty() {
        return Err(ConfigReadError::NoConfigPaths);
    }

    let Some(chosen_config_path) = find_first_existing_path(config_paths) else {
        return Err(ConfigReadError::ConfigNotFound);
    };

    let config = read_cli_config_file(&chosen_config_path)?;

    Ok(config.into_config(cli_arg_address, cli_arg_password))
}

pub fn read_config_for_debuginfo(
    cli_arg_config_path: Option<&Path>,
    cli_arg_address: Option<String>,
    cli_arg_password: Option<String>,
) -> Result<(ConfigFile, Config, PathBuf), ConfigReadError> {
    let config_paths = config_paths(cli_arg_config_path);
    if config_paths.is_empty() {
        return Err(ConfigReadError::NoConfigPaths);
    }

    let Some(chosen_config_path) = find_first_existing_path(config_paths) else {
        return Err(ConfigReadError::ConfigNotFound);
    };

    let config = read_config_file(&chosen_config_path)?;

    Ok((
        config.clone(),
        config.into_config(UiConfig::default(), cli_arg_address, cli_arg_password, false)?,
        chosen_config_path,
    ))
}

pub struct ConfigResult {
    pub config: Config,
    pub config_path: PathBuf,
}

pub fn read_config_and_theme(args: &mut Args) -> Result<ConfigResult, ConfigReadError> {
    let config_paths = config_paths(args.config.as_deref());
    if config_paths.is_empty() {
        return Err(ConfigReadError::NoConfigPaths);
    }

    let Some(chosen_config_path) = find_first_existing_path(config_paths) else {
        return Err(ConfigReadError::ConfigNotFound);
    };

    let config = read_config_file(&chosen_config_path)?;

    let theme = match &config.theme {
        Some(theme_name) => {
            let theme_paths = theme_paths(args.theme.as_deref(), &chosen_config_path, theme_name);
            let chosen_theme_path = find_first_existing_path(theme_paths);

            if let Some(theme_path) = chosen_theme_path {
                read_theme_file(&theme_path)?
            } else {
                return Err(ConfigReadError::ThemeNotFound);
            }
        }
        // No theme set in the config file, this is OK, use the default theme
        None => UiConfigFile::default(),
    };

    let theme = theme.try_into().map_err(ConfigReadError::Conversion)?;

    Ok(ConfigResult {
        config: config
            .into_config(theme, args.address.take(), args.password.take(), false)
            .map_err(ConfigReadError::Conversion)?,
        config_path: chosen_config_path,
    })
}

pub fn read_cli_config_file(path: &Path) -> Result<CliConfigFile, ConfigReadError> {
    let file = std::fs::File::open(path)?;
    let mut read = std::io::BufReader::new(file);
    let mut buf = Vec::new();
    read.read_to_end(&mut buf)?;

    Ok(serde_path_to_error::deserialize(&mut ron::de::Deserializer::from_bytes(&buf)?)?)
}

pub fn read_config_file(path: &Path) -> Result<ConfigFile, ConfigReadError> {
    let file = std::fs::File::open(path)?;
    let mut read = std::io::BufReader::new(file);
    let mut buf = Vec::new();
    read.read_to_end(&mut buf)?;

    Ok(serde_path_to_error::deserialize(&mut ron::de::Deserializer::from_bytes(&buf)?)?)
}

pub fn read_theme_file(path: &Path) -> Result<UiConfigFile, ConfigReadError> {
    let file = std::fs::File::open(path)?;
    let mut read = std::io::BufReader::new(file);
    let mut buf = Vec::new();
    read.read_to_end(&mut buf)?;

    Ok(serde_path_to_error::deserialize(&mut ron::de::Deserializer::from_bytes(&buf)?)?)
}

pub fn find_first_existing_path(paths: Vec<PathBuf>) -> Option<PathBuf> {
    paths.into_iter().find(|path| path.exists())
}
