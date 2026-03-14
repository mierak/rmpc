use std::path::PathBuf;

pub struct LuaPluginEntry {
    pub path: PathBuf,
    pub args: String,
}

impl LuaPluginEntry {
    pub fn new(path: PathBuf, args: String) -> Self {
        Self { path, args }
    }
}
