use std::path::{Path, PathBuf};

use crate::{
    env::ENV,
    paths::utils::{env_var_expand, tilde_expand},
};

#[cfg(debug_assertions)]
const CONFIG_NAME: &str = "config.debug.ron";
#[cfg(not(debug_assertions))]
const CONFIG_NAME: &str = "config.ron";
const CRATE_NAME: &str = env!("CARGO_CRATE_NAME");

pub fn home_dir() -> Option<PathBuf> {
    ENV.var_os("HOME")
        .and_then(|home| if home.is_empty() { None } else { Some(home) })
        .map(PathBuf::from)
}

pub fn config_dir() -> Option<PathBuf> {
    ENV.var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home_dir().map(|home| home.join(".config")))
}

pub fn rmpc_config_dir() -> Option<PathBuf> {
    config_dir().map(|config_dir| config_dir.join("rmpc"))
}

pub fn rmpcd_config_dir() -> Option<PathBuf> {
    config_dir().map(|config_dir| config_dir.join("rmpcd"))
}

pub fn config_paths(cli_arg_config_path: Option<&Path>) -> Vec<PathBuf> {
    if let Some(path) = cli_arg_config_path {
        return vec![path.to_path_buf()];
    }

    let mut result = Vec::new();
    match rmpc_config_dir() {
        Some(config_dir) => result.push(config_dir.join(CONFIG_NAME)),
        None => log::warn!("Could not determine configuration directory"),
    }

    if let Some(home) = home_dir() {
        result.push(home.join(CRATE_NAME).join(CONFIG_NAME));
    }

    result
}

/// # Panics
/// Panics if the config path doesn't have a parent directory
pub fn theme_paths(
    cli_arg_theme: Option<&Path>,
    config_path: &Path,
    theme_name: &str,
) -> Vec<PathBuf> {
    if let Some(path) = cli_arg_theme {
        return vec![path.to_path_buf()];
    }

    let config_dir = config_path.parent().unwrap_or_else(|| {
        panic!("Expected config path to have parent directory. Path: '{}'", config_path.display())
    });

    vec![
        config_dir.join("themes").join(format!("{theme_name}.ron")),
        config_dir.join("themes").join(theme_name),
        config_dir.join(format!("{theme_name}.ron")),
        config_dir.join(theme_name),
        PathBuf::from(tilde_expand(&env_var_expand(theme_name)).into_owned()),
    ]
}

pub mod utils {
    use std::{
        borrow::Cow,
        path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR, Path, PathBuf},
    };

    use anyhow::{Result, anyhow};

    use crate::env::ENV;

    pub fn absolute_env_var_expand_path(inp: &Path) -> Result<Option<PathBuf>> {
        let path_str = inp.to_str().ok_or_else(|| anyhow!("Invalid path: '{}'", inp.display()))?;
        let expanded = env_var_expand(path_str);
        let expanded_path = tilde_expand_path(&PathBuf::from(expanded));
        if expanded_path.is_absolute() {
            return Ok(Some(expanded_path));
        }
        Err(anyhow!("Path is not absolute: {}", expanded_path.display()))
    }

    pub fn tilde_expand_path(inp: &Path) -> PathBuf {
        let Ok(home) = ENV.var("HOME") else {
            return inp.to_owned();
        };
        let home = home.strip_suffix(MAIN_SEPARATOR).unwrap_or(home.as_ref());

        if let Ok(inp) = inp.strip_prefix("~") {
            if inp.as_os_str().is_empty() {
                return home.into();
            }

            return PathBuf::from(home.to_owned()).join(inp);
        }

        inp.to_path_buf()
    }

    pub fn tilde_expand(inp: &str) -> Cow<'_, str> {
        let Ok(home) = ENV.var("HOME") else {
            return Cow::Borrowed(inp);
        };
        let home = home.strip_suffix(MAIN_SEPARATOR).unwrap_or(home.as_ref());

        if let Some(inp) = inp.strip_prefix('~') {
            if inp.is_empty() {
                return Cow::Owned(home.to_owned());
            }

            if inp.starts_with(MAIN_SEPARATOR) {
                return Cow::Owned(format!("{home}{inp}"));
            }
        }

        Cow::Borrowed(inp)
    }

    pub fn env_var_expand(inp: &str) -> String {
        let parts: Vec<&str> = inp.split(MAIN_SEPARATOR).collect();

        let expanded_parts: Vec<String> = parts
            .iter()
            .map(|part| {
                if let Some(var_key) = part.strip_prefix('$') {
                    ENV.var(var_key).unwrap_or_else(|_| (*part).to_string())
                } else {
                    (*part).to_string()
                }
            })
            .collect();

        return expanded_parts.join(MAIN_SEPARATOR_STR);
    }

    #[cfg(test)]
    #[cfg(feature = "test-impl")]
    #[allow(clippy::unwrap_used)]
    mod tests {
        use std::{
            path::PathBuf,
            sync::{LazyLock, Mutex},
        };

        use test_case::test_case;

        use super::{tilde_expand, *};

        static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

        #[test_case("~", "/home/some_user")]
        #[test_case("~enene", "~enene")]
        #[test_case("~nope/", "~nope/")]
        #[test_case("~/yes", "/home/some_user/yes")]
        #[test_case("no/~/no", "no/~/no")]
        #[test_case("basic/path", "basic/path")]
        fn home_dir_present(input: &str, expected: &str) {
            let _guard = TEST_LOCK.lock().unwrap();

            ENV.clear();
            ENV.set("HOME".to_string(), "/home/some_user".to_string());
            assert_eq!(tilde_expand(input), expected);
        }

        #[test_case("~", "~")]
        #[test_case("~enene", "~enene")]
        #[test_case("~nope/", "~nope/")]
        #[test_case("~/yes", "~/yes")]
        #[test_case("no/~/no", "no/~/no")]
        #[test_case("basic/path", "basic/path")]
        fn home_dir_not_present(input: &str, expected: &str) {
            let _guard = TEST_LOCK.lock().unwrap();

            ENV.clear();
            ENV.remove("HOME");
            assert_eq!(tilde_expand(input), expected);
        }

        #[test_case("~", "/home/some_user")]
        #[test_case("~enene", "~enene")]
        #[test_case("~nope/", "~nope/")]
        #[test_case("~/yes", "/home/some_user/yes")]
        #[test_case("no/~/no", "no/~/no")]
        #[test_case("basic/path", "basic/path")]
        fn home_dir_present_path(input: &str, expected: &str) {
            let _guard = TEST_LOCK.lock().unwrap();

            ENV.clear();
            ENV.set("HOME".to_string(), "/home/some_user".to_string());

            let got = tilde_expand_path(&PathBuf::from(input));
            assert_eq!(got, PathBuf::from(expected));
        }

        #[test_case("~", "~")]
        #[test_case("~enene", "~enene")]
        #[test_case("~nope/", "~nope/")]
        #[test_case("~/yes", "~/yes")]
        #[test_case("no/~/no", "no/~/no")]
        #[test_case("basic/path", "basic/path")]
        fn home_dir_not_present_path(input: &str, expected: &str) {
            let _guard = TEST_LOCK.lock().unwrap();

            ENV.clear();
            ENV.remove("HOME");

            let got = tilde_expand_path(&PathBuf::from(input));
            assert_eq!(got, PathBuf::from(expected));
        }

        #[test_case("$HOME", "/home/some_user")]
        #[test_case("$HOME/yes", "/home/some_user/yes")]
        #[test_case("start/$VALUE/end", "start/path/end")]
        #[test_case("$EMPTY/path", "/path")]
        #[test_case("start/$EMPTY/end", "start//end")]
        #[test_case("$NOT_SET", "$NOT_SET")]
        #[test_case("no/$NOT_SET/path", "no/$NOT_SET/path")]
        #[test_case("basic/path", "basic/path")]
        // NOTE: current implementation only expands vars that are the entire part.
        // This is different from how shells do it, but I can't think of a use case for
        // it in paths #[test_case("no$HOME$VALUE", "no/home/some_userpath")]
        fn env_var_expansion(input: &str, expected: &str) {
            let _guard = TEST_LOCK.lock().unwrap();

            ENV.clear();
            ENV.set("HOME".to_string(), "/home/some_user".to_string());
            ENV.set("VALUE".to_string(), "path".to_string());
            ENV.set("EMPTY".to_string(), String::new());
            assert_eq!(env_var_expand(input), expected);
        }

        #[test_case("$HOME", "/home/some_user")]
        #[test_case("$HOME/yes", "/home/some_user/yes")]
        #[test_case("/start/$VALUE/end", "/start/path/end")]
        #[test_case("$EMPTY/path", "/path")]
        #[test_case("/start/$EMPTY/end", "/start//end")]
        #[test_case("/$NOT_SET", "/$NOT_SET")]
        #[test_case("/basic/path", "/basic/path")]
        fn env_var_expansion_path(input: &str, expected: &str) {
            let _guard = TEST_LOCK.lock().unwrap();

            ENV.clear();
            ENV.set("HOME".to_string(), "/home/some_user".to_string());
            ENV.set("VALUE".to_string(), "path".to_string());
            ENV.set("EMPTY".to_string(), String::new());
            let got = absolute_env_var_expand_path(PathBuf::from(input).as_path()).ok().unwrap();
            assert_eq!(got, Some(PathBuf::from(expected)));
        }
    }
}
