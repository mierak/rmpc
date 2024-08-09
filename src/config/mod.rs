use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use cli::{Args, OnOff, OnOffOneshot};
use serde::{Deserialize, Serialize};

pub mod cli;
mod defaults;
pub mod keys;
pub mod theme;

use crate::utils::image_proto::{self, ImageProtocol};
use crate::utils::macros::status_warn;
use crate::utils::tmux;

use self::{
    keys::{KeyConfig, KeyConfigFile},
    theme::{ConfigColor, UiConfig, UiConfigFile},
};

#[derive(Default, Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum ImageMethodFile {
    Kitty,
    UeberzugWayland,
    UeberzugX11,
    Iterm2,
    None,
    #[default]
    Auto,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMethod {
    Kitty,
    UeberzugWayland,
    UeberzugX11,
    Iterm2,
    None,
    #[default]
    Unsupported,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

impl Default for Size {
    fn default() -> Self {
        Self {
            width: 600,
            height: 600,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Config {
    pub address: &'static str,
    pub cache_dir: Option<&'static str>,
    pub volume_step: u8,
    pub keybinds: KeyConfig,
    pub status_update_interval_ms: Option<u64>,
    pub select_current_song_on_change: bool,
    pub theme: UiConfig,
    pub album_art: AlbumArtConfig,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigFile {
    address: String,
    #[serde(default)]
    cache_dir: Option<String>,
    #[serde(default)]
    theme: Option<String>,
    #[serde(default = "defaults::default_volume_step")]
    volume_step: u8,
    #[serde(default = "defaults::default_progress_update_interval_ms")]
    status_update_interval_ms: Option<u64>,
    #[serde(default = "defaults::default_false")]
    select_current_song_on_change: bool,
    #[serde(default)]
    keybinds: KeyConfigFile,
    #[serde(default)]
    image_method: Option<ImageMethodFile>,
    #[serde(default)]
    album_art_max_size_px: Size,
    #[serde(default)]
    album_art: AlbumArtConfigFile,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AlbumArtConfigFile {
    #[serde(default)]
    pub method: ImageMethodFile,
    #[serde(default)]
    pub max_size_px: Size,
}

#[derive(Debug, Default, Clone)]
pub struct AlbumArtConfig {
    pub method: ImageMethod,
    pub max_size_px: Size,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            address: String::from("127.0.0.1:6600"),
            keybinds: KeyConfigFile::default(),
            volume_step: 5,
            status_update_interval_ms: Some(1000),
            theme: None,
            cache_dir: None,
            image_method: None,
            select_current_song_on_change: false,
            album_art_max_size_px: Size::default(),
            album_art: AlbumArtConfigFile::default(),
        }
    }
}

impl ConfigFile {
    pub fn read(path: &PathBuf, address: Option<String>) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let read = std::io::BufReader::new(file);
        let mut config: ConfigFile = ron::de::from_reader(read)?;
        if let Some(address) = address {
            config.address = address;
        }

        Ok(config)
    }

    fn read_theme(&self, config_dir: &Path) -> Result<UiConfigFile> {
        self.theme.as_ref().map_or_else(
            || Ok(UiConfigFile::default()),
            |theme_name| -> Result<_> {
                let path = PathBuf::from(config_dir)
                    .join("themes")
                    .join(format!("{theme_name}.ron"));
                let file = std::fs::File::open(&path)
                    .with_context(|| format!("Failed to open theme file {:?}", path.to_string_lossy()))?;
                let read = std::io::BufReader::new(file);
                let theme: UiConfigFile = ron::de::from_reader(read)?;
                Ok(theme)
            },
        )
    }

    pub fn into_config(self, config_dir: Option<&Path>) -> Result<Config> {
        let theme: UiConfig = config_dir
            .map(|d| self.read_theme(d.parent().expect("Config path to be defined correctly")))
            .transpose()?
            .unwrap_or_default()
            .try_into()?;

        let size = self.album_art.max_size_px;
        let mut config = Config {
            theme,
            cache_dir: self.cache_dir.map(|v| -> &'static str {
                if v.ends_with('/') {
                    Box::leak(Box::new(v))
                } else {
                    Box::leak(Box::new(format!("{v}/")))
                }
            }),
            address: Box::leak(Box::new(self.address)),
            volume_step: self.volume_step,
            status_update_interval_ms: self.status_update_interval_ms.map(|v| v.max(100)),
            keybinds: self.keybinds.into(),
            select_current_song_on_change: self.select_current_song_on_change,
            album_art: AlbumArtConfig {
                method: ImageMethod::default(),
                max_size_px: Size {
                    width: if size.width == 0 { u16::MAX } else { size.width },
                    height: if size.height == 0 { u16::MAX } else { size.height },
                },
            },
        };

        let is_tmux = tmux::is_inside_tmux();
        if is_tmux && !tmux::is_passthrough_enabled()? {
            tmux::enable_passthrough()?;
        };
        config.album_art.method = if config.theme.album_art_width_percent == 0 {
            ImageMethod::None
        } else {
            match self.image_method.unwrap_or(self.album_art.method) {
                ImageMethodFile::Iterm2 if image_proto::is_iterm2_supported(is_tmux) => ImageMethod::Iterm2,
                ImageMethodFile::Iterm2 => ImageMethod::Unsupported,
                ImageMethodFile::Kitty if image_proto::is_kitty_supported(is_tmux)? => ImageMethod::Kitty,
                ImageMethodFile::Kitty => ImageMethod::Unsupported,
                ImageMethodFile::UeberzugWayland if image_proto::is_ueberzug_wayland_supported() => {
                    ImageMethod::UeberzugWayland
                }
                ImageMethodFile::UeberzugWayland => ImageMethod::Unsupported,
                ImageMethodFile::UeberzugX11 if image_proto::is_ueberzug_x11_supported() => ImageMethod::UeberzugX11,
                ImageMethodFile::UeberzugX11 => ImageMethod::Unsupported,
                ImageMethodFile::None => ImageMethod::None,
                ImageMethodFile::Auto if config.theme.album_art_width_percent == 0 => ImageMethod::None,
                ImageMethodFile::Auto => match image_proto::determine_image_support(is_tmux)? {
                    ImageProtocol::Kitty => ImageMethod::Kitty,
                    ImageProtocol::UeberzugWayland => ImageMethod::UeberzugWayland,
                    ImageProtocol::UeberzugX11 => ImageMethod::UeberzugX11,
                    ImageProtocol::Iterm2 => ImageMethod::Iterm2,
                    ImageProtocol::None => ImageMethod::Unsupported,
                },
            }
        };

        match config.album_art.method {
            ImageMethod::Unsupported => {
                status_warn!(
                    "Album art is enabled but no image protocol is supported by your terminal, disabling album art"
                );
                config.theme.album_art_width_percent = 0;
            }
            ImageMethod::None => {
                config.theme.album_art_width_percent = 0;
            }
            ImageMethod::Kitty | ImageMethod::UeberzugWayland | ImageMethod::UeberzugX11 | ImageMethod::Iterm2 => {
                log::debug!(requested:? = self.album_art.method, resolved:? = config.album_art.method, is_tmux; "Image method resolved");
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {

    use walkdir::WalkDir;

    #[cfg(debug_assertions)]
    use crate::config::keys::KeyConfigFile;
    use crate::config::{theme::UiConfigFile, ConfigFile};

    #[test]
    #[cfg(debug_assertions)]
    fn example_config_equals_default() {
        let config = ConfigFile::default();
        let path = format!(
            "{}/assets/example_config.ron",
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
            "{}/assets/example_config.ron",
            std::env::var("CARGO_MANIFEST_DIR").unwrap()
        );

        let f: ConfigFile = ron::de::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

        assert_eq!(config, f);
    }

    #[test]
    fn example_theme_equals_default() {
        let theme = UiConfigFile::default();
        let path = format!(
            "{}/assets/example_theme.ron",
            std::env::var("CARGO_MANIFEST_DIR").unwrap()
        );

        let file = ron::de::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

        assert_eq!(theme, file);
    }

    #[test]
    fn gallery_themes_are_valid() {
        let path = format!(
            "{}/docs/src/assets/themes",
            std::env::var("CARGO_MANIFEST_DIR").unwrap()
        );

        for entry in WalkDir::new(path).follow_links(true).into_iter().filter_map(Result::ok) {
            let f_name = entry.file_name().to_string_lossy();

            if f_name.ends_with(".ron") {
                dbg!(entry.path());
                ron::de::from_str::<UiConfigFile>(&std::fs::read_to_string(entry.path()).unwrap()).unwrap();
            }
        }
    }
}
