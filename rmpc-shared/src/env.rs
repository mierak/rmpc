use std::{
    ffi::{OsStr, OsString},
    sync::LazyLock,
};

pub struct Env {
    #[cfg(feature = "test-impl")]
    vars: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
}

pub static ENV: LazyLock<Env> = LazyLock::new(|| Env {
    #[cfg(feature = "test-impl")]
    vars: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::default())),
});

#[cfg(not(feature = "test-impl"))]
impl Env {
    pub fn var<K: AsRef<OsStr>>(&self, key: K) -> Result<String, std::env::VarError> {
        std::env::var(key)
    }

    pub fn var_os<K: AsRef<OsStr>>(&self, key: K) -> Option<OsString> {
        std::env::var_os(key)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn set(&self, _key: impl Into<String>, _value: impl Into<String>) {}

    pub fn clear(&self) {}

    pub fn remove(&self, _key: &str) {}
}

#[allow(clippy::unwrap_used)]
#[cfg(feature = "test-impl")]
#[allow(clippy::missing_panics_doc)]
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

    pub fn set(&self, key: impl Into<String>, value: impl Into<String>) {
        self.vars.lock().unwrap().insert(key.into(), value.into());
    }

    pub fn clear(&self) {
        self.vars.lock().unwrap().clear();
    }

    pub fn remove(&self, key: impl AsRef<str>) {
        self.vars.lock().unwrap().remove(key.as_ref());
    }
}
