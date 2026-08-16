use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub author: String,
    pub name: String,
    pub entry_point: PathBuf,
}

impl Manifest {
    pub async fn read(plugin_dir: &Path) -> Result<Manifest> {
        let manifest_path = plugin_dir.join("manifest.json");
        let manifest = tokio::fs::read_to_string(&manifest_path)
            .await
            .inspect_err(|err| {
                if err.kind() == ErrorKind::NotFound {
                    tracing::error!("Plugin manifest not found at {}", manifest_path.display());
                }
            })
            .with_context(|| {
                format!("Failed to read plugin manifest at {}", manifest_path.display())
            })?;

        let manifest: Manifest = serde_json::from_str(&manifest)
            .with_context(|| format!("Failed to deserialize manifest.json: '{manifest}'"))?;

        Ok(manifest)
    }
}
