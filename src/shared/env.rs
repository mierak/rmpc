use std::ffi::{OsStr, OsString};
use std::sync::LazyLock;

pub struct Env {
    #[cfg(test)]
    vars: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
}

pub static ENV: LazyLock<Env> = LazyLock::new(|| Env {
    #[cfg(test)]
    vars: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::default())),
});

#[cfg(not(test))]
impl Env {
    pub fn var<K: AsRef<OsStr>>(&self, key: K) -> Result<String, std::env::VarError> {
        std::env::var(key)
    }

    pub fn var_os<K: AsRef<OsStr>>(&self, key: K) -> Option<OsString> {
        std::env::var_os(key)
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
impl Env {
    pub fn var<K: AsRef<OsStr>>(&self, key: K) -> Result<String, std::env::VarError> {
        let Some(key) = key.as_ref().to_str() else {
            return Err(std::env::VarError::NotUnicode("".into()));
        };

        self.vars.lock().unwrap().get(key).cloned().ok_or(std::env::VarError::NotPresent)
    }

    pub fn var_os<K: AsRef<OsStr>>(&self, key: K) -> Option<OsString> {
        key.as_ref()
            .to_str()
            .and_then(|v| self.vars.lock().unwrap().get(v).cloned())
            .map(|v| v.into())
    }

    pub fn set(&self, key: String, value: String) {
        self.vars.lock().unwrap().insert(key, value);
    }

    pub fn clear(&self) {
        self.vars.lock().unwrap().clear();
    }

    pub fn remove(&self, key: &str) {
        self.vars.lock().unwrap().remove(key);
    }
}
