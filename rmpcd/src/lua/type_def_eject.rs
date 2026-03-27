use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rmpc_shared::{paths::rmpcd_data_dir, version::Version};
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use sha2::{Digest, Sha256};
use tracing::{info, trace};

include!(concat!(env!("OUT_DIR"), "/lua_type_defs.rs"));

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    #[serde_as(as = "DisplayFromStr")]
    rmpcd_version: Version,
    hash: String,
    files: Vec<PathBuf>,
}

fn hash_type_defs() -> String {
    let mut hasher = Sha256::new();
    for def in TYPE_DEFS {
        hasher.update(def.2);
    }
    format!("{:x}", hasher.finalize())
}

pub fn eject() -> Result<()> {
    eject_inner(&RealFs)
}

pub fn eject_inner(fs: &impl FileSystem) -> Result<()> {
    let Some(mut data_dir) = rmpcd_data_dir() else {
        bail!("Could not determine data directory");
    };

    data_dir.push("lua");

    let hash = hash_type_defs();
    let crate_version: Version = env!("CARGO_PKG_VERSION").parse()?;
    let manifest_path = data_dir.clone().join("manifest.json");

    if fs.exists(&data_dir) {
        let manifest = fs.read_str(&manifest_path).with_context(|| {
            format!("Failed to read manifest file at '{}'", manifest_path.display())
        })?;

        let manifest = serde_json::from_str::<Manifest>(&manifest).with_context(|| {
            format!(
                "Failed to parse manifest file at '{}' with content '{}'",
                manifest_path.display(),
                manifest
            )
        })?;

        if manifest.rmpcd_version > crate_version {
            info!(
                "Lua type definitions manifest is from newer rmpcd version ({}), skipping eject",
                manifest.rmpcd_version
            );
            return Ok(());
        }

        if manifest.hash != hash {
            info!("Lua type definitions have changed, updating...");
            for file in manifest.files {
                let path = data_dir.join(&file);
                trace!(path = ?path.display(), "Removing old lua type definition file");
                fs.remove_file(&path).with_context(|| {
                    format!(
                        "Failed to remove existing lua type definition file at '{}'",
                        path.display()
                    )
                })?;
            }

            eject_type_defs(fs, crate_version, hash, &data_dir)
                .context("Failed to eject lua type definitions")?;

            return Ok(());
        }

        info!("Lua type definitions are up to date, skipping eject");
    } else {
        info!(
            "Lua type definitions not found, ejecting type definitions to {}",
            data_dir.display()
        );
        eject_type_defs(fs, crate_version, hash, &data_dir)
            .context("Failed to eject lua type definitions")?;
    }

    Ok(())
}

fn eject_type_defs(
    fs: &impl FileSystem,
    crate_version: Version,
    hash: String,
    data_dir: &Path,
) -> Result<()> {
    let mut manifest = Manifest { rmpcd_version: crate_version, hash, files: Vec::new() };

    info!("Ejecting Lua type definitions to {}", data_dir.display());

    for (dir, filename, content) in TYPE_DEFS {
        let mut path = data_dir.to_owned();
        if !dir.is_empty() {
            for subdir in *dir {
                path.push(subdir);
            }
        }

        fs.create_dir_all(&path).with_context(|| {
            format!("Failed to create directory for lua type definitions at '{}'", path.display())
        })?;

        path.push(filename);
        manifest.files.push(path.strip_prefix(data_dir)?.to_path_buf());

        trace!(path = ?path.display(), "Writing lua type definition file at");
        fs.write(&path, content).with_context(|| {
            format!("Failed to write lua type definition file at '{}'", path.display())
        })?;
    }

    fs.write(&data_dir.join("manifest.json"), serde_json::to_string(&manifest)?.as_bytes())
        .context("Failed to write type definition manifest")?;

    Ok(())
}

pub trait FileSystem {
    type Err: std::error::Error + Send + Sync + 'static;
    fn exists(&self, path: &Path) -> bool;
    fn read_str(&self, path: &Path) -> Result<String, Self::Err>;
    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), Self::Err>;
    fn remove_file(&self, path: &Path) -> Result<(), Self::Err>;
    fn create_dir_all(&self, path: &Path) -> Result<(), Self::Err>;
}

pub struct RealFs;
impl FileSystem for RealFs {
    type Err = std::io::Error;

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn read_str(&self, path: &Path) -> Result<String, Self::Err> {
        std::fs::read_to_string(path)
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), Self::Err> {
        std::fs::write(path, contents)
    }

    fn remove_file(&self, path: &Path) -> Result<(), Self::Err> {
        std::fs::remove_file(path)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), Self::Err> {
        std::fs::create_dir_all(path)
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod test {
    use std::{
        cell::RefCell,
        collections::{HashMap, VecDeque},
    };

    use anyhow::anyhow;
    use rmpc_shared::env::ENV;

    use super::*;

    #[derive(Default)]
    struct TestFs {
        calls: RefCell<HashMap<String, Vec<String>>>,
        exists: RefCell<VecDeque<bool>>,
        read_str: RefCell<VecDeque<Result<String>>>,
        write_contents: RefCell<HashMap<String, Vec<u8>>>,
    }

    impl TestFs {
        fn stub_exists(&self, responses: bool) {
            let mut exists = self.exists.borrow_mut();
            exists.push_back(responses);
        }

        fn stub_read_str(&self, response: Result<impl Into<String>>) {
            let mut read_str = self.read_str.borrow_mut();
            read_str.push_back(response.map(Into::into));
        }
    }

    impl FileSystem for TestFs {
        type Err = std::io::Error;

        fn exists(&self, path: &Path) -> bool {
            self.calls
                .borrow_mut()
                .entry("exists".to_string())
                .or_default()
                .push(path.display().to_string());

            self.exists.borrow_mut().pop_front().unwrap()
        }

        fn read_str(&self, path: &Path) -> Result<String, Self::Err> {
            self.calls
                .borrow_mut()
                .entry("read_str".to_string())
                .or_default()
                .push(path.display().to_string());

            self.read_str
                .borrow_mut()
                .pop_front()
                .unwrap()
                .map_err(|err| std::io::Error::other(err.to_string()))
        }

        fn write(&self, path: &Path, contents: &[u8]) -> Result<(), Self::Err> {
            self.calls
                .borrow_mut()
                .entry("write".to_string())
                .or_default()
                .push(path.display().to_string());
            self.write_contents.borrow_mut().insert(path.display().to_string(), contents.to_vec());
            Ok(())
        }

        fn remove_file(&self, path: &Path) -> Result<(), Self::Err> {
            self.calls
                .borrow_mut()
                .entry("remove_file".to_string())
                .or_default()
                .push(path.display().to_string());
            Ok(())
        }

        fn create_dir_all(&self, path: &Path) -> Result<(), Self::Err> {
            self.calls
                .borrow_mut()
                .entry("create_dir_all".to_string())
                .or_default()
                .push(path.display().to_string());
            Ok(())
        }
    }

    macro_rules! calls {
        ($fs:expr, $method:expr) => {
            $fs.calls.borrow().get($method).unwrap_or(&Vec::new())
        };
    }

    #[test]
    fn fails_on_missing_home() {
        let _lock = ENV.lock();
        let fs = TestFs::default();

        ENV.clear();
        fs.stub_exists(false);

        let result = eject_inner(&fs);

        assert_eq!(result.unwrap_err().to_string(), "Could not determine data directory");
    }

    #[test]
    fn uses_xdg_data_dir() {
        let _lock = ENV.lock();
        let fs = TestFs::default();

        ENV.clear();
        ENV.set("XDG_DATA_HOME", "/tmp/test_data");
        fs.stub_exists(false);

        let result = eject_inner(&fs);

        assert!(
            calls!(fs, "write").contains(&"/tmp/test_data/rmpcd/lua/manifest.json".to_string()),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn fails_with_missing_manifest() {
        let _lock = ENV.lock();
        let fs = TestFs::default();

        ENV.clear();
        ENV.set("HOME", "/home/user");
        fs.stub_exists(true);
        fs.stub_read_str(Err::<&str, _>(anyhow!("manifest not found")));

        let result = eject_inner(&fs);

        assert_eq!(
            result.unwrap_err().to_string(),
            "Failed to read manifest file at '/home/user/.local/share/rmpcd/lua/manifest.json'"
        );
    }

    #[test]
    fn fails_with_invalid_manifest() {
        let _lock = ENV.lock();
        let fs = TestFs::default();

        ENV.clear();
        ENV.set("HOME", "/home/user");
        fs.stub_exists(true);
        fs.stub_read_str(Ok("invalid json"));

        let result = eject_inner(&fs);

        assert_eq!(
            result.unwrap_err().to_string(),
            "Failed to parse manifest file at '/home/user/.local/share/rmpcd/lua/manifest.json' with content 'invalid json'"
        );
    }

    #[test]
    fn does_not_eject_when_up_to_date() {
        let _lock = ENV.lock();
        let fs = TestFs::default();
        let hash = hash_type_defs();
        let crate_version: Version = env!("CARGO_PKG_VERSION").parse().unwrap();

        ENV.clear();
        ENV.set("HOME", "/home/user");
        fs.stub_exists(true);
        fs.stub_read_str(Ok(format!(
            r#"{{"rmpcd_version":"{crate_version}","hash":"{hash}","files":["rmpcd.lua"]}}"#
        )));

        let result = eject_inner(&fs);

        assert!(result.is_ok());
        assert!(calls!(fs, "remove_file").is_empty());
        assert!(calls!(fs, "write").is_empty());
        assert!(calls!(fs, "create_dir_all").is_empty());
    }

    #[test]
    fn does_not_eject_when_higher_version() {
        let _lock = ENV.lock();
        let fs = TestFs::default();
        let crate_version: Version = Version::new(99, 99, 99);

        ENV.clear();
        ENV.set("HOME", "/home/user");
        fs.stub_exists(true);
        fs.stub_read_str(Ok(format!(
            r#"{{"rmpcd_version":"{crate_version}","hash":"does not match","files":["rmpcd.lua"]}}"#
        )));

        let result = eject_inner(&fs);

        assert!(result.is_ok());
        assert!(calls!(fs, "remove_file").is_empty());
        assert!(calls!(fs, "write").is_empty());
        assert!(calls!(fs, "create_dir_all").is_empty());
    }

    #[test]
    fn ejects_when_hash_mismatch() {
        let _lock = ENV.lock();
        let fs = TestFs::default();
        let crate_version: Version = env!("CARGO_PKG_VERSION").parse().unwrap();

        ENV.clear();
        ENV.set("HOME", "/home/user");
        fs.stub_exists(true);
        fs.stub_read_str(Ok(format!(
            r#"{{"rmpcd_version":"{crate_version}","hash":"does not match","files":["rmpcd.lua"]}}"#
        )));

        let result = eject_inner(&fs);

        assert!(result.is_ok());
        assert_eq!(calls!(fs, "remove_file").len(), 1);
        assert_eq!(calls!(fs, "write").len(), 15);
        for f in [
            "/home/user/.local/share/rmpcd/lua/rmpcd.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/log.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/lyrics.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/util.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/http.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/process.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/lastfm.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/mpd/song.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/mpd/init.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/mpd/status.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/playcount.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/fs.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/notify.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/sync.lua",
            "/home/user/.local/share/rmpcd/lua/manifest.json",
        ] {
            assert!(calls!(fs, "write").contains(&f.to_string()), "Expected write call for '{f}'");
        }
        assert_eq!(calls!(fs, "create_dir_all").len(), 14);
    }

    #[test]
    fn ejects_when_dir_does_not_exist() {
        let _lock = ENV.lock();
        let fs = TestFs::default();

        ENV.clear();
        ENV.set("HOME", "/home/user");
        fs.stub_exists(false);

        let result = eject_inner(&fs);

        assert!(result.is_ok());
        assert_eq!(calls!(fs, "write").len(), 15);
        for f in [
            "/home/user/.local/share/rmpcd/lua/rmpcd.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/log.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/lyrics.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/util.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/http.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/process.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/lastfm.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/mpd/song.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/mpd/init.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/mpd/status.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/playcount.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/fs.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/notify.lua",
            "/home/user/.local/share/rmpcd/lua/rmpcd/sync.lua",
            "/home/user/.local/share/rmpcd/lua/manifest.json",
        ] {
            assert!(calls!(fs, "write").contains(&f.to_string()), "Expected write call for '{f}'");
        }

        assert_eq!(calls!(fs, "create_dir_all").len(), 14);
    }

    #[test]
    fn writes_correct_manifest() {
        let _lock = ENV.lock();
        let fs = TestFs::default();

        ENV.clear();
        ENV.set("HOME", "/home/user");
        fs.stub_exists(false);
        let hash = hash_type_defs();
        let crate_version: Version = env!("CARGO_PKG_VERSION").parse().unwrap();

        let result = eject_inner(&fs);

        assert!(result.is_ok());
        let m: Manifest = serde_json::from_str(&String::from_utf8_lossy(
            fs.write_contents
                .borrow()
                .get("/home/user/.local/share/rmpcd/lua/manifest.json")
                .unwrap(),
        ))
        .unwrap();
        assert_eq!(m.rmpcd_version, crate_version);
        assert_eq!(m.hash, hash);
        assert_eq!(m.files.len(), 14);
        for f in [
            "rmpcd.lua",
            "rmpcd/log.lua",
            "rmpcd/lyrics.lua",
            "rmpcd/util.lua",
            "rmpcd/http.lua",
            "rmpcd/process.lua",
            "rmpcd/lastfm.lua",
            "rmpcd/mpd/song.lua",
            "rmpcd/mpd/init.lua",
            "rmpcd/mpd/status.lua",
            "rmpcd/playcount.lua",
            "rmpcd/fs.lua",
            "rmpcd/notify.lua",
            "rmpcd/sync.lua",
        ] {
            assert!(m.files.contains(&PathBuf::from(f)), "Expected manifest to contain file '{f}'");
        }
    }
}
