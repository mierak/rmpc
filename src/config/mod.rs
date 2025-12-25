use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

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
pub use search::Search;

use self::{
    keys::{KeyConfig, KeyConfigFile},
    theme::{ConfigColor, UiConfig, UiConfigFile},
};
use crate::{
    config::{
        tabs::{SizedPaneOrSplit, Tab, TabName},
        utils::tilde_expand_path,
    },
    shared::{lrc::LrcOffset, terminal::TERMINAL},
    tmux,
};

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Clone)]
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
pub struct ConfigFile {
    #[serde(default = "defaults::mpd_address")]
    pub address: String,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    cache_dir: Option<PathBuf>,
    #[serde(default)]
    lyrics_dir: Option<String>,
    #[serde(default = "defaults::i64::<0>")]
    lyrics_offset_ms: i64,
    #[serde(default = "defaults::bool::<true>")]
    enable_lyrics_index: bool,
    #[serde(default = "defaults::bool::<false>")]
    enable_lyrics_hot_reload: bool,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default = "defaults::u8::<5>")]
    volume_step: u8,
    #[serde(default = "defaults::u32::<30>")]
    pub max_fps: u32,
    #[serde(default = "defaults::usize::<0>")]
    scrolloff: usize,
    #[serde(default = "defaults::bool::<false>")]
    wrap_navigation: bool,
    #[serde(default = "defaults::default_progress_update_interval_ms")]
    status_update_interval_ms: Option<u64>,
    #[serde(default = "defaults::bool::<false>")]
    select_current_song_on_change: bool,
    #[serde(default = "defaults::bool::<false>")]
    center_current_song_on_change: bool,
    #[serde(default = "defaults::bool::<false>")]
    reflect_changes_to_playlist: bool,
    #[serde(default)]
    rewind_to_start_sec: Option<u64>,
    #[serde(default = "defaults::bool::<true>")]
    keep_state_on_song_change: bool,
    #[serde(default = "defaults::u64::<10_000>")]
    mpd_read_timeout_ms: u64,
    #[serde(default = "defaults::u64::<5_000>")]
    mpd_write_timeout_ms: u64,
    #[serde(default)]
    mpd_idle_read_timeout_ms: Option<u64>,
    #[serde(default = "defaults::bool::<true>")]
    enable_mouse: bool,
    #[serde(default = "defaults::bool::<true>")]
    pub enable_config_hot_reload: bool,
    #[serde(default)]
    keybinds: KeyConfigFile,
    #[serde(default = "defaults::u64::<1000>")]
    pub normal_timeout_ms: u64,
    #[serde(default = "defaults::u64::<1000>")]
    pub insert_timeout_ms: u64,
    // Deprecated
    #[serde(default)]
    image_method: Option<ImageMethodFile>,
    #[serde(default)]
    album_art_max_size_px: Size,
    #[serde(default)]
    pub album_art: AlbumArtConfigFile,
    #[serde(default)]
    on_song_change: Option<Vec<String>>,
    #[serde(default)]
    exec_on_song_change_at_start: bool,
    #[serde(default)]
    on_resize: Option<Vec<String>>,
    #[serde(default)]
    search: SearchFile,
    #[serde(default)]
    artists: ArtistsFile,
    #[serde(default)]
    tabs: TabsFile,
    #[serde(default)]
    pub ignore_leading_the: bool,
    #[serde(default)]
    pub browser_song_sort: Vec<SongPropertyFile>,
    #[serde(default)]
    pub show_playlists_in_browser: ShowPlaylistsMode,
    #[serde(default)]
    pub directories_sort: SortModeFile,
    #[serde(default)]
    pub cava: CavaFile,
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
                disabled_protocols: defaults::disabled_album_art_protos(),
                ..Default::default()
            },
            on_song_change: None,
            exec_on_song_change_at_start: false,
            on_resize: None,
            search: SearchFile::default(),
            tabs: TabsFile::default(),
            enable_mouse: true,
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
}

#[derive(Debug)]
pub enum DeserError {
    Deserialization(serde_path_to_error::Error<ron::Error>),
    NotFound(std::io::Error),
    Io(std::io::Error),
    Ron(ron::error::SpannedError),
    Generic(anyhow::Error),
}

impl std::error::Error for DeserError {}
impl std::fmt::Display for DeserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeserError::Deserialization(err) => {
                write!(
                    f,
                    "Failed to deserialize config at path: '{}'.\nError: '{}'.",
                    err.path(),
                    err.inner()
                )
            }
            DeserError::NotFound(err) => write!(f, "Failed to read config file. Error: '{err}'"),
            DeserError::Io(err) => write!(f, "Failed to read config file. Error: '{err}'"),
            DeserError::Ron(err) => {
                write!(f, "Failed to parse config file. Error: '{err}'")
            }
            DeserError::Generic(err) => write!(f, "Failed to read config file. Error: '{err:#}'"),
        }
    }
}

impl From<std::io::Error> for DeserError {
    fn from(value: std::io::Error) -> Self {
        if value.kind() == std::io::ErrorKind::NotFound {
            Self::NotFound(value)
        } else {
            Self::Io(value)
        }
    }
}

impl From<ron::error::SpannedError> for DeserError {
    fn from(value: ron::error::SpannedError) -> Self {
        Self::Ron(value)
    }
}

impl From<serde_path_to_error::Error<ron::Error>> for DeserError {
    fn from(value: serde_path_to_error::Error<ron::Error>) -> Self {
        Self::Deserialization(value)
    }
}

impl From<anyhow::Error> for DeserError {
    fn from(value: anyhow::Error) -> Self {
        Self::Generic(value)
    }
}

impl ConfigFile {
    pub fn read(path: &PathBuf) -> Result<Self, DeserError> {
        let file = std::fs::File::open(path)?;
        let mut read = std::io::BufReader::new(file);
        let mut buf = Vec::new();
        read.read_to_end(&mut buf)?;
        let result: Result<ConfigFile, _> =
            serde_path_to_error::deserialize(&mut ron::de::Deserializer::from_bytes(&buf)?);

        Ok(result?)
    }

    pub fn theme_path(&self, config_dir: &Path) -> Option<PathBuf> {
        self.theme.as_ref().and_then(|theme| {
            let theme_paths = [
                config_dir.join("themes").join(format!("{theme}.ron")),
                config_dir.join("themes").join(theme),
                config_dir.join(format!("{theme}.ron")),
                config_dir.join(theme),
                PathBuf::from(tilde_expand(theme).into_owned()),
            ];
            theme_paths.into_iter().find(|theme_path| theme_path.is_file())
        })
    }

    fn read_theme(&self, config_dir: &Path) -> Result<UiConfigFile, DeserError> {
        self.theme_path(config_dir).map_or_else(
            || Ok(UiConfigFile::default()),
            |path| {
                let file = std::fs::File::open(&path)?;
                let mut read = std::io::BufReader::new(file);
                let mut buf = Vec::new();
                read.read_to_end(&mut buf)?;
                let theme: UiConfigFile = serde_path_to_error::deserialize(
                    &mut ron::de::Deserializer::from_bytes(&buf)?,
                )?;

                Ok(theme)
            },
        )
    }

    pub fn into_config(
        self,
        config_path: Option<&Path>,
        theme_cli: Option<&Path>,
        address_cli: Option<String>,
        password_cli: Option<String>,
        skip_album_art_check: bool,
    ) -> Result<Config, DeserError> {
        let theme = if let Some(path) = theme_cli {
            let file = std::fs::File::open(path).with_context(|| {
                format!("Failed to open theme file {:?}", path.to_string_lossy())
            })?;
            let read = std::io::BufReader::new(file);
            ron::de::from_reader(read)?
        } else if let Some(path) = config_path {
            let config_dir = path.parent().with_context(|| {
                format!("Expected config path to have parent directory. Path: '{}'", path.display())
            })?;

            self.read_theme(config_dir)?
        } else {
            UiConfigFile::default()
        };

        let theme = UiConfig::try_from(theme)?;

        let original_tabs_definition = self.tabs.clone();
        let tabs: Tabs = self.tabs.convert(&theme.components)?;
        let active_panes = Config::calc_active_panes(&tabs.tabs, &theme.layout);

        let (address, password) =
            MpdAddress::resolve(address_cli, password_cli, self.address, self.password);
        let album_art_method = self.album_art.method;
        let mut config = Config {
            theme_name: self.theme,
            cache_dir: self.cache_dir.map(|v| tilde_expand_path(&v)),
            lyrics_dir: self.lyrics_dir.map(|v| {
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
        path::{MAIN_SEPARATOR, Path, PathBuf},
    };

    use crate::shared::env::ENV;

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
        let home = home.strip_suffix("/").unwrap_or(home.as_ref());

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

    #[cfg(test)]
    #[allow(clippy::unwrap_used)]
    mod tests {
        use std::{
            path::PathBuf,
            sync::{LazyLock, Mutex},
        };

        use test_case::test_case;

        use super::tilde_expand;
        use crate::{config::utils::tilde_expand_path, shared::env::ENV};

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
            ENV.set("HOME".to_string(), "/home/some_user/".to_string());
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
            ENV.set("HOME".to_string(), "/home/some_user/".to_string());

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
