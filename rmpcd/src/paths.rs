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
    pub cache: PathBuf,
    pub plugins: PathBuf,
    pub type_def: PathBuf,
    pub albumart_cache: PathBuf,
    pub runtime: PathBuf,
}

#[allow(dead_code)]
impl Paths {
    fn new() -> Result<Self> {
        let Some(config) = rmpcd_config_dir() else {
            bail!("Could not determine config directory");
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

        Ok(Self {
            config: config.canonicalize().context("Failed to canonicalize config directory")?,
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

    pub fn init() -> Result<&'static Self> {
        let paths = Self::new()?;
        PATHS.set(paths).map_err(|_| anyhow::anyhow!("Paths already initialized"))?;
        Ok(Self::get())
    }

    pub fn config_dir() -> &'static Path {
        Self::get().config.as_path()
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
