use std::{collections::HashMap, path::PathBuf, str::FromStr, sync::Arc, time::Duration};

use address::MpdPassword;
use album_art::{AlbumArtConfig, AlbumArtConfigFile, ImageMethodFile};
use anyhow::{Context, Result};
use artists::{Artists, ArtistsFile};
use cava::{Cava, CavaFile};
use clap::Parser;
use cli::{Args, OnOff, OnOffOneshot};
use itertools::Itertools;
use search::SearchFile;
use serde::{Deserialize, Serialize};
use sort_mode::{SortMode, SortModeFile, SortOptions};
use tabs::{PaneType, Tabs, TabsFile, validate_tabs};
use theme::properties::{SongProperty, SongPropertyFile};
use utils::tilde_expand;

pub mod address;
pub mod album_art;
pub mod artists;
pub mod cava;
pub mod cli;
pub mod cli_config;
mod defaults;
pub mod keys;
mod search;
pub mod sort_mode;
pub mod tabs;
pub mod theme;

pub use address::MpdAddress;
pub use search::{FilterKindFile, Search};

use self::{
    keys::{KeyConfig, KeyConfigFile},
    theme::{ConfigColor, UiConfig},
};
use crate::{
    config::{
        tabs::{SizedPaneOrSplit, Tab, TabName},
        utils::{absolute_env_var_expand_path, env_var_expand},
    },
    shared::{duration_format::DurationFormat, lrc::LrcOffset, terminal::TERMINAL},
    tmux,
};

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct Config {
    pub address: MpdAddress,
    pub password: Option<MpdPassword>,
    pub cache_dir: Option<PathBuf>,
    pub lyrics_dir: Option<String>,
    pub lyrics_offset: LrcOffset,
    pub enable_lyrics_index: bool,
    pub enable_lyrics_hot_reload: bool,
    pub volume_step: u8,
    pub max_fps: u32,
    pub scrolloff: usize,
    pub wrap_navigation: bool,
    pub keybinds: KeyConfig,
    pub normal_timeout_ms: u64,
    pub insert_timeout_ms: u64,
    pub enable_mouse: bool,
    pub scroll_amount: usize,
    pub enable_config_hot_reload: bool,
    pub status_update_interval_ms: Option<u64>,
    pub select_current_song_on_change: bool,
    pub center_current_song_on_change: bool,
    pub reflect_changes_to_playlist: bool,
    pub rewind_to_start_sec: Option<u64>,
    pub keep_state_on_song_change: bool,
    pub mpd_read_timeout: Duration,
    pub mpd_write_timeout: Duration,
    pub mpd_idle_read_timeout_ms: Option<Duration>,
    pub theme: UiConfig,
    pub theme_name: Option<String>,
    pub album_art: AlbumArtConfig,
    pub on_song_change: Option<Arc<Vec<String>>>,
    pub exec_on_song_change_at_start: bool,
    pub on_resize: Option<Arc<Vec<String>>>,
    pub search: Search,
    pub artists: Artists,
    pub tabs: Tabs,
    pub original_tabs_definition: TabsFile,
    pub active_panes: Vec<PaneType>,
    pub browser_song_sort: Arc<SortOptions>,
    pub show_playlists_in_browser: ShowPlaylistsMode,
    pub directories_sort: Arc<SortOptions>,
    pub cava: Cava,
    pub auto_open_downloads: bool,
    pub extra_yt_dlp_args: Vec<String>,
    pub duration_format: DurationFormat,
}

impl Default for Config {
    fn default() -> Self {
        ConfigFile::default()
            .into_config(UiConfig::default(), None, None, true)
            .expect("Default config should be valid")
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ShowPlaylistsMode {
    All,
    None,
    #[default]
    NonRoot,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ConfigFile {
    pub address: String,
    password: Option<String>,
    cache_dir: Option<PathBuf>,
    lyrics_dir: Option<String>,
    lyrics_offset_ms: i64,
    enable_lyrics_index: bool,
    enable_lyrics_hot_reload: bool,
    pub theme: Option<String>,
    volume_step: u8,
    pub max_fps: u32,
    scrolloff: usize,
    wrap_navigation: bool,
    status_update_interval_ms: Option<u64>,
    select_current_song_on_change: bool,
    center_current_song_on_change: bool,
    reflect_changes_to_playlist: bool,
    rewind_to_start_sec: Option<u64>,
    keep_state_on_song_change: bool,
    mpd_read_timeout_ms: u64,
    mpd_write_timeout_ms: u64,
    mpd_idle_read_timeout_ms: Option<u64>,
    enable_mouse: bool,
    scroll_amount: usize,
    pub enable_config_hot_reload: bool,
    keybinds: KeyConfigFile,
    pub normal_timeout_ms: u64,
    pub insert_timeout_ms: u64,
    // Deprecated
    image_method: Option<ImageMethodFile>,
    album_art_max_size_px: Size,
    pub album_art: AlbumArtConfigFile,
    on_song_change: Option<Vec<String>>,
    exec_on_song_change_at_start: bool,
    on_resize: Option<Vec<String>>,
    search: SearchFile,
    artists: ArtistsFile,
    tabs: TabsFile,
    pub ignore_leading_the: bool,
    pub browser_song_sort: Vec<SongPropertyFile>,
    pub show_playlists_in_browser: ShowPlaylistsMode,
    pub directories_sort: SortModeFile,
    pub cava: CavaFile,
    pub extra_yt_dlp_args: Vec<String>,
    pub auto_open_downloads: bool,
    pub duration_format: String,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

impl Default for Size {
    fn default() -> Self {
        Self { width: 1200, height: 1200 }
    }
}

impl From<(u16, u16)> for Size {
    fn from(value: (u16, u16)) -> Self {
        Self { width: value.0, height: value.1 }
    }
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            address: String::from("127.0.0.1:6600"),
            keybinds: KeyConfigFile::default(),
            normal_timeout_ms: 1000,
            insert_timeout_ms: 1000,
            volume_step: 5,
            scrolloff: 0,
            status_update_interval_ms: Some(1000),
            mpd_write_timeout_ms: 5_000,
            mpd_read_timeout_ms: 10_000,
            mpd_idle_read_timeout_ms: None,
            max_fps: 30,
            theme: None,
            cache_dir: None,
            lyrics_dir: None,
            lyrics_offset_ms: 0,
            enable_lyrics_index: true,
            enable_lyrics_hot_reload: false,
            image_method: None,
            select_current_song_on_change: false,
            center_current_song_on_change: false,
            album_art_max_size_px: Size::default(),
            album_art: AlbumArtConfigFile {
                disabled_protocols: vec!["http://".to_string(), "https://".to_string()],
                ..Default::default()
            },
            on_song_change: None,
            exec_on_song_change_at_start: false,
            on_resize: None,
            search: SearchFile::default(),
            tabs: TabsFile::default(),
            enable_mouse: true,
            scroll_amount: 1,
            enable_config_hot_reload: true,
            wrap_navigation: false,
            password: None,
            artists: ArtistsFile::default(),
            ignore_leading_the: false,
            browser_song_sort: defaults::default_song_sort(),
            directories_sort: SortModeFile::SortFormat { group_by_type: true, reverse: false },
            rewind_to_start_sec: None,
            keep_state_on_song_change: true,
            reflect_changes_to_playlist: false,
            cava: CavaFile::default(),
            show_playlists_in_browser: ShowPlaylistsMode::default(),
            extra_yt_dlp_args: Vec::new(),
            auto_open_downloads: true,
            duration_format: "%m:%S".to_string(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        validate_tabs(&self.theme.layout, &self.tabs)
    }

    pub fn calc_active_panes(
        tabs: &HashMap<TabName, Tab>,
        layout: &SizedPaneOrSplit,
    ) -> Vec<PaneType> {
        tabs.iter()
            .flat_map(|(_, tab)| tab.panes.panes_iter().map(|pane| pane.pane.clone()))
            .chain(layout.panes_iter().map(|pane| pane.pane.clone()))
            .unique()
            .collect_vec()
    }

    pub fn default_cli(args: &mut Args) -> Config {
        ConfigFile::default()
            .into_config(
                UiConfig::default(),
                std::mem::take(&mut args.address),
                std::mem::take(&mut args.password),
                false,
            )
            .expect("Default config should always convert")
    }

    pub fn default_with_album_art_check() -> Result<Config> {
        ConfigFile::default().into_config(UiConfig::default(), None, None, false)
    }
}

impl ConfigFile {
    pub fn into_config(
        self,
        theme: UiConfig,
        address_cli: Option<String>,
        password_cli: Option<String>,
        skip_album_art_check: bool,
    ) -> Result<Config, anyhow::Error> {
        let original_tabs_definition = self.tabs.clone();
        let tabs: Tabs = self.tabs.convert(&theme.components, &theme.border_symbol_sets)?;
        let active_panes = Config::calc_active_panes(&tabs.tabs, &theme.layout);

        let (address, password) =
            MpdAddress::resolve(address_cli, password_cli, self.address, self.password);
        let album_art_method = self.album_art.method;
        let mut config = Config {
            theme_name: self.theme,
            cache_dir: self
                .cache_dir
                .map_or(Ok(None), |v| -> Result<Option<PathBuf>> {
                    absolute_env_var_expand_path(&v)
                })
                .context("Invalid cache_dir path")?,
            lyrics_dir: self.lyrics_dir.map(|v| {
                let v = env_var_expand(&v);
                let v = tilde_expand(&v);
                if v.ends_with('/') { v.into_owned() } else { format!("{v}/") }
            }),
            lyrics_offset: LrcOffset::from_millis(self.lyrics_offset_ms),
            enable_lyrics_index: self.enable_lyrics_index,
            enable_lyrics_hot_reload: self.enable_lyrics_hot_reload,
            tabs,
            original_tabs_definition,
            active_panes,
            address,
            password,
            volume_step: self.volume_step,
            max_fps: self.max_fps,
            scrolloff: self.scrolloff,
            wrap_navigation: self.wrap_navigation,
            status_update_interval_ms: self.status_update_interval_ms.map(|v| v.max(100)),
            mpd_read_timeout: Duration::from_millis(self.mpd_read_timeout_ms),
            mpd_write_timeout: Duration::from_millis(self.mpd_write_timeout_ms),
            mpd_idle_read_timeout_ms: self.mpd_idle_read_timeout_ms.map(Duration::from_millis),
            enable_mouse: self.enable_mouse,
            scroll_amount: self.scroll_amount,
            enable_config_hot_reload: self.enable_config_hot_reload,
            normal_timeout_ms: self.normal_timeout_ms,
            insert_timeout_ms: self.insert_timeout_ms,
            keybinds: self.keybinds.try_into()?,
            select_current_song_on_change: self.select_current_song_on_change,
            center_current_song_on_change: self.center_current_song_on_change,
            search: self.search.try_into()?,
            artists: self.artists.into(),
            album_art: self.album_art.into(),
            on_song_change: self.on_song_change.map(|arr| {
                Arc::new(arr.into_iter().map(|v| tilde_expand(&v).into_owned()).collect_vec())
            }),
            exec_on_song_change_at_start: self.exec_on_song_change_at_start,
            on_resize: self.on_resize.map(|arr| {
                Arc::new(arr.into_iter().map(|v| tilde_expand(&v).into_owned()).collect_vec())
            }),
            show_playlists_in_browser: self.show_playlists_in_browser,
            browser_song_sort: Arc::new(SortOptions {
                mode: SortMode::Format(
                    self.browser_song_sort.iter().cloned().map(SongProperty::from).collect_vec(),
                ),
                group_by_type: true,
                reverse: false,
                ignore_leading_the: self.ignore_leading_the,
                fold_case: true,
            }),
            directories_sort: Arc::new(match self.directories_sort {
                SortModeFile::Format { group_by_type, reverse } => SortOptions {
                    mode: SortMode::Format(
                        theme
                            .browser_song_format
                            .0
                            .iter()
                            .flat_map(|prop| prop.kind.collect_properties())
                            .collect_vec(),
                    ),
                    group_by_type,
                    reverse,
                    ignore_leading_the: self.ignore_leading_the,
                    fold_case: true,
                },
                SortModeFile::SortFormat { group_by_type, reverse } => SortOptions {
                    mode: SortMode::Format(
                        self.browser_song_sort.into_iter().map(SongProperty::from).collect_vec(),
                    ),
                    group_by_type,
                    reverse,
                    ignore_leading_the: self.ignore_leading_the,
                    fold_case: true,
                },
                SortModeFile::ModifiedTime { group_by_type, reverse } => SortOptions {
                    mode: SortMode::ModifiedTime,
                    group_by_type,
                    reverse,
                    ignore_leading_the: self.ignore_leading_the,
                    fold_case: true,
                },
            }),
            theme,
            rewind_to_start_sec: self.rewind_to_start_sec,
            keep_state_on_song_change: self.keep_state_on_song_change,
            reflect_changes_to_playlist: self.reflect_changes_to_playlist,
            cava: self.cava.into(),
            extra_yt_dlp_args: self.extra_yt_dlp_args,
            auto_open_downloads: self.auto_open_downloads,
            duration_format: DurationFormat::parse(&self.duration_format)?,
        };

        if skip_album_art_check {
            return Ok(config);
        }

        let is_tmux = tmux::is_inside_tmux();
        if is_tmux && !tmux::is_passthrough_enabled()? {
            tmux::enable_passthrough()?;
        }

        config.album_art.method =
            TERMINAL.resolve_image_backend(self.image_method.unwrap_or(album_art_method));

        Ok(config)
    }
}

impl FromStr for Args {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Args::try_parse_from(std::iter::once("").chain(s.split_whitespace()))?)
    }
}

impl From<OnOff> for bool {
    fn from(value: OnOff) -> Self {
        match value {
            OnOff::On => true,
            OnOff::Off => false,
        }
    }
}

impl From<OnOffOneshot> for crate::mpd::commands::status::OnOffOneshot {
    fn from(value: OnOffOneshot) -> Self {
        match value {
            OnOffOneshot::On => crate::mpd::commands::status::OnOffOneshot::On,
            OnOffOneshot::Off => crate::mpd::commands::status::OnOffOneshot::Off,
            OnOffOneshot::Oneshot => crate::mpd::commands::status::OnOffOneshot::Oneshot,
        }
    }
}

pub mod utils {
    use std::{
        borrow::Cow,
        path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR, Path, PathBuf},
    };

    use crate::shared::env::ENV;

    pub fn absolute_env_var_expand_path(inp: &Path) -> Result<Option<PathBuf>, anyhow::Error> {
        let path_str =
            inp.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path: '{}'", inp.display()))?;
        let expanded = env_var_expand(path_str);
        let expanded_path = tilde_expand_path(&PathBuf::from(expanded));
        if expanded_path.is_absolute() {
            return Ok(Some(expanded_path));
        }
        Err(anyhow::anyhow!("Path is not absolute: {}", expanded_path.display()))
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
    #[allow(clippy::unwrap_used)]
    mod tests {
        use std::{
            path::PathBuf,
            sync::{LazyLock, Mutex},
        };

        use test_case::test_case;

        use super::tilde_expand;
        use crate::{
            config::utils::{absolute_env_var_expand_path, env_var_expand, tilde_expand_path},
            shared::env::ENV,
        };

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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    #[cfg(debug_assertions)]
    use crate::config::keys::KeyConfigFile;
    use crate::config::{ConfigFile, theme::UiConfigFile};

    #[test]
    #[cfg(debug_assertions)]
    fn example_config_equals_default() {
        let config = ConfigFile::default();
        let path =
            format!("{}/assets/example_config.ron", std::env::var("CARGO_MANIFEST_DIR").unwrap());

        let mut f: ConfigFile = ron::de::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        f.keybinds.logs = KeyConfigFile::default().logs;

        assert_eq!(config, f);
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn example_config_equals_default() {
        let config = ConfigFile::default();
        let path =
            format!("{}/assets/example_config.ron", std::env::var("CARGO_MANIFEST_DIR").unwrap());

        let f: ConfigFile = ron::de::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

        assert_eq!(config, f);
    }

    #[test]
    fn example_theme_equals_default() {
        let theme = UiConfigFile::default();
        let path =
            format!("{}/assets/example_theme.ron", std::env::var("CARGO_MANIFEST_DIR").unwrap());

        let file = ron::de::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

        assert_eq!(theme, file);
    }
}
