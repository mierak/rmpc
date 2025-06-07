use std::{
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use address::MpdPassword;
use album_art::{AlbumArtConfig, AlbumArtConfigFile, ImageMethod, ImageMethodFile};
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
    shared::{image, image::ImageProtocol, macros::status_warn},
    tmux,
};

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default, Clone)]
pub struct Config {
    pub address: MpdAddress,
    pub password: Option<MpdPassword>,
    pub cache_dir: Option<PathBuf>,
    pub lyrics_dir: Option<String>,
    pub volume_step: u8,
    pub max_fps: u32,
    pub scrolloff: usize,
    pub wrap_navigation: bool,
    pub keybinds: KeyConfig,
    pub enable_mouse: bool,
    pub enable_config_hot_reload: bool,
    pub status_update_interval_ms: Option<u64>,
    pub select_current_song_on_change: bool,
    pub center_current_song_on_change: bool,
    pub reflect_changes_to_playlist: bool,
    pub rewind_to_start_sec: Option<u64>,
    pub mpd_read_timeout: Duration,
    pub mpd_write_timeout: Duration,
    pub mpd_idle_read_timeout_ms: Option<Duration>,
    pub theme: UiConfig,
    pub theme_name: Option<String>,
    pub album_art: AlbumArtConfig,
    pub on_song_change: Option<Arc<Vec<String>>>,
    pub on_resize: Option<Arc<Vec<String>>>,
    pub search: Search,
    pub artists: Artists,
    pub tabs: Tabs,
    pub active_panes: Vec<PaneType>,
    pub browser_song_sort: Arc<SortOptions>,
    pub directories_sort: Arc<SortOptions>,
    pub cava: Cava,
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
    #[serde(default)]
    image_method: Option<ImageMethodFile>,
    #[serde(default)]
    album_art_max_size_px: Size,
    #[serde(default)]
    pub album_art: AlbumArtConfigFile,
    #[serde(default)]
    on_song_change: Option<Vec<String>>,
    #[serde(default)]
    on_resize: Option<Vec<String>>,
    #[serde(default)]
    search: SearchFile,
    #[serde(default)]
    artists: ArtistsFile,
    #[serde(default)]
    tabs: TabsFile,
    #[serde(default)]
    pub browser_song_sort: Vec<SongPropertyFile>,
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
            image_method: None,
            select_current_song_on_change: false,
            center_current_song_on_change: false,
            album_art_max_size_px: Size::default(),
            album_art: AlbumArtConfigFile {
                disabled_protocols: defaults::disabled_album_art_protos(),
                ..Default::default()
            },
            on_song_change: None,
            on_resize: None,
            search: SearchFile::default(),
            tabs: TabsFile::default(),
            enable_mouse: true,
            enable_config_hot_reload: true,
            wrap_navigation: false,
            password: None,
            artists: ArtistsFile::default(),
            browser_song_sort: defaults::default_song_sort(),
            directories_sort: SortModeFile::SortFormat {
                group_directories_first: true,
                reverse: false,
            },
            rewind_to_start_sec: None,
            reflect_changes_to_playlist: false,
            cava: CavaFile::default(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        validate_tabs(&self.theme.layout, &self.tabs)
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
            DeserError::Generic(err) => write!(f, "Failed to read config file. Error: '{err}'"),
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
        self.theme.as_ref().map(|theme_name| {
            PathBuf::from(config_dir).join("themes").join(format!("{theme_name}.ron"))
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

        let tabs: Tabs = self.tabs.convert(&theme.components)?;
        let active_panes = tabs
            .tabs
            .iter()
            .flat_map(|(_, tab)| tab.panes.panes_iter().map(|pane| pane.pane.clone()))
            .chain(theme.layout.panes_iter().map(|pane| pane.pane.clone()))
            .unique()
            .collect_vec();

        let (address, password) =
            MpdAddress::resolve(address_cli, password_cli, self.address, self.password);
        let album_art_method = self.album_art.method;
        let mut config = Config {
            theme_name: self.theme,
            cache_dir: self.cache_dir,
            lyrics_dir: self.lyrics_dir.map(|v| {
                let v = tilde_expand(&v);
                if v.ends_with('/') { v.into_owned() } else { format!("{v}/") }
            }),
            tabs,
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
            keybinds: self.keybinds.into(),
            select_current_song_on_change: self.select_current_song_on_change,
            center_current_song_on_change: self.center_current_song_on_change,
            search: self.search.into(),
            artists: self.artists.into(),
            album_art: self.album_art.into(),
            on_song_change: self.on_song_change.map(|arr| {
                Arc::new(arr.into_iter().map(|v| tilde_expand(&v).into_owned()).collect_vec())
            }),
            on_resize: self.on_resize.map(|arr| {
                Arc::new(arr.into_iter().map(|v| tilde_expand(&v).into_owned()).collect_vec())
            }),
            browser_song_sort: Arc::new(SortOptions {
                mode: SortMode::Format(
                    self.browser_song_sort.iter().cloned().map(SongProperty::from).collect_vec(),
                ),
                group_directories_first: true,
                reverse: false,
            }),
            directories_sort: Arc::new(match self.directories_sort {
                SortModeFile::Format { group_directories_first, reverse } => SortOptions {
                    mode: SortMode::Format(
                        theme
                            .browser_song_format
                            .0
                            .iter()
                            .flat_map(|prop| prop.kind.collect_properties())
                            .collect_vec(),
                    ),
                    group_directories_first,
                    reverse,
                },
                SortModeFile::SortFormat { group_directories_first, reverse } => SortOptions {
                    mode: SortMode::Format(
                        self.browser_song_sort.into_iter().map(SongProperty::from).collect_vec(),
                    ),
                    group_directories_first,
                    reverse,
                },
                SortModeFile::ModifiedTime { group_directories_first, reverse } => {
                    SortOptions { mode: SortMode::ModifiedTime, group_directories_first, reverse }
                }
            }),
            theme,
            rewind_to_start_sec: self.rewind_to_start_sec,
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

        config.album_art.method = match self.image_method.unwrap_or(album_art_method) {
            ImageMethodFile::Iterm2 => ImageMethod::Iterm2,
            ImageMethodFile::Kitty => ImageMethod::Kitty,
            ImageMethodFile::UeberzugWayland if image::is_ueberzug_wayland_supported() => {
                ImageMethod::UeberzugWayland
            }
            ImageMethodFile::UeberzugWayland => ImageMethod::Unsupported,
            ImageMethodFile::UeberzugX11 if image::is_ueberzug_x11_supported() => {
                ImageMethod::UeberzugX11
            }
            ImageMethodFile::UeberzugX11 => ImageMethod::Unsupported,
            ImageMethodFile::Sixel => ImageMethod::Sixel,
            ImageMethodFile::Block => ImageMethod::Block,
            ImageMethodFile::None => ImageMethod::None,
            ImageMethodFile::Auto => match image::determine_image_support(is_tmux)? {
                ImageProtocol::Kitty => ImageMethod::Kitty,
                ImageProtocol::UeberzugWayland => ImageMethod::UeberzugWayland,
                ImageProtocol::UeberzugX11 => ImageMethod::UeberzugX11,
                ImageProtocol::Iterm2 => ImageMethod::Iterm2,
                ImageProtocol::Sixel => ImageMethod::Sixel,
                ImageProtocol::Block => ImageMethod::Block,
                ImageProtocol::None => ImageMethod::Unsupported,
            },
        };

        match config.album_art.method {
            ImageMethod::Unsupported => {
                status_warn!(
                    "Album art is enabled but no image protocol is supported by your terminal, disabling album art"
                );
            }
            ImageMethod::None => {}
            ImageMethod::Kitty
            | ImageMethod::UeberzugWayland
            | ImageMethod::UeberzugX11
            | ImageMethod::Iterm2
            | ImageMethod::Sixel
            | ImageMethod::Block => {
                log::debug!(resolved:? = config.album_art.method, requested:? = album_art_method, is_tmux; "Image method resolved");
            }
        }

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
    use std::{borrow::Cow, path::MAIN_SEPARATOR};

    use crate::shared::env::ENV;

    pub fn tilde_expand(inp: &str) -> Cow<str> {
        let Ok(home) = ENV.var("HOME") else {
            return Cow::Borrowed(inp);
        };

        if let Some(inp) = inp.strip_prefix('~') {
            if inp.is_empty() {
                return Cow::Owned(home);
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
        use std::sync::{LazyLock, Mutex};

        use test_case::test_case;

        use super::tilde_expand;
        use crate::shared::env::ENV;

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
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {

    use walkdir::WalkDir;

    #[cfg(debug_assertions)]
    use crate::config::keys::KeyConfigFile;
    use crate::config::{ConfigFile, theme::UiConfigFile};

    #[test]
    #[cfg(debug_assertions)]
    fn example_config_equals_default() {
        let config = ConfigFile::default();
        let path = format!(
            "{}/docs/src/content/docs/next/assets/example_config.ron",
            std::env::var("CARGO_MANIFEST_DIR").unwrap()
        );

        let mut f: ConfigFile = ron::de::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        f.keybinds.logs = KeyConfigFile::default().logs;

        assert_eq!(config, f);
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn example_config_equals_default() {
        let config = ConfigFile::default();
        let path = format!(
            "{}/docs/src/content/docs/next/assets/example_config.ron",
            std::env::var("CARGO_MANIFEST_DIR").unwrap()
        );

        let f: ConfigFile = ron::de::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

        assert_eq!(config, f);
    }

    #[test]
    fn example_theme_equals_default() {
        let theme = UiConfigFile::default();
        let path = format!(
            "{}/docs/src/content/docs/next/assets/example_theme.ron",
            std::env::var("CARGO_MANIFEST_DIR").unwrap()
        );

        let file = ron::de::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

        assert_eq!(theme, file);
    }

    #[test]
    fn gallery_themes_are_valid() {
        let path = format!(
            "{}/docs/src/content/docs/next/assets/themes",
            std::env::var("CARGO_MANIFEST_DIR").unwrap()
        );

        for entry in WalkDir::new(path).follow_links(true).into_iter().filter_map(Result::ok) {
            let f_name = entry.file_name().to_string_lossy();

            if f_name.ends_with("theme.ron") {
                dbg!(entry.path());
                ron::de::from_str::<UiConfigFile>(&std::fs::read_to_string(entry.path()).unwrap())
                    .unwrap();
            }
        }
    }
}
