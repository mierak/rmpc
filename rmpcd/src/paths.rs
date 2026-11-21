use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Context, Result, bail};
use rmpc_shared::paths::{rmpcd_cache_dir, rmpcd_config_dir, rmpcd_data_dir, runtime_dir};

static PATHS: OnceLock<Paths> = OnceLock::new();

#[allow(dead_code)]
pub struct Paths {
    pub config: PathBuf,
    pub config_file: PathBuf,
    pub config_file_overridden: bool,
    pub cache: PathBuf,
    pub plugins: PathBuf,
    pub type_def: PathBuf,
    pub albumart_cache: PathBuf,
    pub runtime: PathBuf,
}

#[allow(dead_code)]
impl Paths {
    fn new(config_file: Option<PathBuf>) -> Result<Self> {
        let config_file_overridden = config_file.is_some();
        let (config, config_file_name) = if let Some(path) = config_file {
            if path.is_dir() {
                (path, "init.lua".into())
            } else {
                let Some(name) = path.file_name() else {
                    bail!("Invalid config file path '{}'", path.display());
                };
                let dir = path.parent().filter(|dir| !dir.as_os_str().is_empty());
                (dir.unwrap_or(Path::new(".")).to_path_buf(), name.to_owned())
            }
        } else {
            let Some(config) = rmpcd_config_dir() else {
                bail!("Could not determine config directory");
            };
            (config, "init.lua".into())
        };

        let Some(cache) = rmpcd_cache_dir() else {
            bail!("Could not determine cache directory");
        };

        let Some(data) = rmpcd_data_dir() else {
            bail!("Could not determine runtime directory");
        };

        let runtime_dir = runtime_dir().map(|d| d.join("rmpcd")).unwrap_or(cache.clone());
        let plugins = data.join("plugins");
        let type_def = data.join("lua");
        let albumart_cache = cache.join("albumart");

        std::fs::create_dir_all(&config).context("Failed to create config directory")?;
        std::fs::create_dir_all(&cache).context("Failed to create cache directory")?;
        std::fs::create_dir_all(&data).context("Failed to create data directory")?;
        std::fs::create_dir_all(&plugins).context("Failed to create plugins directory")?;
        std::fs::create_dir_all(&type_def).context("Failed to create type definition directory")?;
        std::fs::create_dir_all(&albumart_cache)
            .context("Failed to create album art cache directory")?;
        std::fs::create_dir_all(&runtime_dir).context("Failed to create runtime directory")?;

        let config = config.canonicalize().context("Failed to canonicalize config directory")?;
        Ok(Self {
            config_file: config.join(config_file_name),
            config_file_overridden,
            config,
            cache: cache.canonicalize().context("Failed to canonicalize cache directory")?,
            plugins: plugins.canonicalize().context("Failed to canonicalize plugins directory")?,
            type_def: type_def
                .canonicalize()
                .context("Failed to canonicalize type def directory")?,
            albumart_cache: albumart_cache
                .canonicalize()
                .context("Failed to canonicalize album art directory")?,
            runtime: runtime_dir
                .canonicalize()
                .context("Failed to canonicalize runtime directory")?,
        })
    }

    pub fn get() -> &'static Self {
        PATHS.get().expect("Paths not initialized")
    }

    pub fn init(config_file: Option<PathBuf>) -> Result<&'static Self> {
        let paths = Self::new(config_file)?;
        PATHS.set(paths).map_err(|_| anyhow::anyhow!("Paths already initialized"))?;
        Ok(Self::get())
    }

    pub fn config_dir() -> &'static Path {
        Self::get().config.as_path()
    }

    pub fn config_file() -> &'static Path {
        Self::get().config_file.as_path()
    }

    pub fn cache_dir() -> &'static Path {
        Self::get().cache.as_path()
    }

    pub fn plugins_dir() -> &'static Path {
        Self::get().plugins.as_path()
    }

    pub fn type_def_dir() -> &'static Path {
        Self::get().type_def.as_path()
    }

    pub fn albumart_cache_dir() -> &'static Path {
        Self::get().albumart_cache.as_path()
    }

    pub fn runtime_dir() -> &'static Path {
        Self::get().runtime.as_path()
    }
}
