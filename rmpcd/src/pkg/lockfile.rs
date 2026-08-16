use std::{collections::BTreeMap, io::ErrorKind};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths::Paths;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct LockfileEntry {
    pub url: String,
    pub branch: Option<String>,
    pub rev: String,
}

#[derive(Default, Debug)]
pub struct Lockfile {
    inner: BTreeMap<String, LockfileEntry>,
    changed: bool,
}

impl Lockfile {
    pub async fn read_or_default() -> Result<Lockfile> {
        let lockfile_path = Paths::plugins_dir().join("rmpcd-lock.json");
        let lockfile = match tokio::fs::read_to_string(&lockfile_path).await {
            Ok(v) => v,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Lockfile::default()),
            err @ Err(_) => err.with_context(|| {
                format!("Faled to read rmpcd-lock.json at '{}'", lockfile_path.display())
            })?,
        };
        let lockfile: BTreeMap<String, LockfileEntry> = serde_json::from_str(&lockfile)
            .with_context(|| format!("Failed to deserialize rmpcd-lock.json: '{lockfile}'"))?;

        Ok(Lockfile { inner: lockfile, changed: false })
    }

    pub async fn write(self) -> Result<()> {
        if !self.changed {
            tracing::info!("No changes to rmpcd-lock.json, skipping write");
            return Ok(());
        }

        let lockfile_path = Paths::plugins_dir().join("rmpcd-lock.json");
        let tmp_lockfile_path = Paths::plugins_dir().join("rmpcd-lock.json.tmp");
        let lockfile = serde_json::to_string_pretty(&self.inner)
            .with_context(|| "Failed to serialize rmpcd-lock.json")?;

        tokio::fs::write(&tmp_lockfile_path, lockfile).await.with_context(|| {
            format!("Failed to write rmpcd-lock.json at '{}'", lockfile_path.display())
        })?;
        tokio::fs::rename(&tmp_lockfile_path, &lockfile_path).await.with_context(|| {
            format!("Failed to rename rmpcd-lock.json.tmp to '{}'", lockfile_path.display())
        })?;

        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&LockfileEntry> {
        self.inner.get(name)
    }

    pub fn record(&mut self, name: String, branch: Option<String>, url: String, rev: String) {
        let entry = LockfileEntry { url, branch, rev };
        if self.inner.get(&name).is_some_and(|e| *e == entry) {
            return;
        }

        self.changed = true;
        self.inner.insert(name, entry);
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&String, &mut LockfileEntry) -> bool,
    {
        let original_len = self.inner.len();
        self.inner.retain(f);
        if self.inner.len() != original_len {
            self.changed = true;
        }
    }
}
