#![allow(clippy::unwrap_used)]
use std::{
    env,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

use walkdir::WalkDir;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    collect_lua_type_defs();
}

fn collect_lua_type_defs() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src").join("lua").join("types");
    let dest_path = Path::new(&env::var_os("OUT_DIR").unwrap()).join("lua_type_defs.rs");

    let walk = WalkDir::new(&input);
    let mut output = String::new();

    output.push_str("const TYPE_DEFS: &[(&[&str], &str, &[u8])] = &[\n");

    for entry in walk {
        let entry = entry.unwrap();
        if entry.metadata().unwrap().is_dir() {
            continue;
        }

        if entry.path().extension().is_none_or(|ext| ext != "lua") {
            continue;
        }

        let _ = write!(output, "(&[");
        for c in entry.path().strip_prefix(&input).unwrap().parent().unwrap().components() {
            let _ = write!(output, "\"{}\", ", c.as_os_str().to_string_lossy());
        }
        let _ = write!(output, "], ");
        let _ = write!(output, "\"{}\", ", entry.file_name().display());
        let _ = writeln!(output, "include_bytes!(\"{}\")),", entry.path().display());
    }

    let _ = writeln!(output, "];");

    fs::write(dest_path, output).unwrap();
    println!("cargo::rerun-if-changed=src/lua/types");
}
