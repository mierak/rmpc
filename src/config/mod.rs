use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::Result;
use anyhow::{bail, Context};
use clap::{Parser, Subcommand, ValueEnum};
use log::Level;
use rustix::path::Arg;
use serde::{Deserialize, Serialize};

mod defaults;
pub mod keys;
pub mod theme;

use crate::mpd::commands::volume::Bound;
use crate::mpd::mpd_client::MpdClient;
use crate::utils::image_proto::ImageProtocol;
use crate::utils::macros::status_warn;
use crate::WorkRequest;

use self::{
    keys::{KeyConfig, KeyConfigFile},
    theme::{ConfigColor, UiConfig, UiConfigFile},
};

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long, value_name = "FILE", default_value = get_default_config_path().into_os_string())]
    pub config: PathBuf,
    #[arg(short, long, default_value_t = Level::Debug)]
    pub log: Level,
    #[command(subcommand)]
    pub command: Option<Command>,
    #[arg(short, long)]
    /// Override the address to connect to. Defaults to value in the config file.
    pub address: Option<String>,
}

#[derive(Subcommand, Clone, Debug, PartialEq)]
#[clap(rename_all = "lower")]
pub enum Command {
    /// Prints the default config. Can be used to bootstrap your config file.
    Config,
    /// Prints the default theme. Can be used to bootstrap your theme file.
    Theme,
    /// Prints the rmpc version
    Version,
    /// Plays song at the position in the current playlist. Defaults to current paused song.
    Play { position: Option<u32> },
    /// Pause playback
    Pause,
    /// Unpause playback
    Unpause,
    /// Toggles between play and pause
    TogglePause,
    /// Stops playback
    Stop,
    /// Plays the next song in the playlist
    Next,
    /// Plays the previous song in the playlist
    Prev,
    /// Sets volume, relative if prefixed by + or -. Prints current volume if no arguments is given.
    Volume {
        #[arg(allow_negative_numbers(true))]
        value: Option<String>,
    },
    /// On or off
    Repeat { value: OnOff },
    /// On or off
    Random { value: OnOff },
    /// On, off or oneshot
    Single { value: OnOffOneshot },
    /// On, off or oneshot
    Consume { value: OnOffOneshot },
    /// Seeks current song(seconds), relative if prefixed by + or -
    Seek {
        #[arg(allow_negative_numbers(true))]
        value: String,
    },
    /// Clear the current queue
    Clear,
    /// Add a song to the current queue. Relative to music database root. '/' to add all files to the queue
    Add { file: String },
    /// Add a song from youtube to the current queue.
    AddYt { url: String },
    /// List MPD outputs
    Outputs,
    /// Toggle MPD output on or off
    ToggleOutput {
        // Id of the output to toggle
        id: u32,
    },
    /// Enable MPD output
    EnableOutput {
        // Id of the output to enable
        id: u32,
    },
    /// Disable MPD output
    DisableOutput {
        // Id of the output to disable
        id: u32,
    },
    /// Prints various information like the playback status
    Status,
    /// Prints information about the current song
    Song,
    /// Mounts supported storage to MPD
    Mount { name: String, path: String },
    /// Unmounts storage with given name
    Unmount { name: String },
    /// List currently mounted storages
    ListMounts,
}

#[derive(Parser, ValueEnum, Copy, Clone, Debug, PartialEq)]
pub enum OnOff {
    On,
    Off,
}

#[derive(Parser, ValueEnum, Copy, Clone, Debug, PartialEq)]
pub enum OnOffOneshot {
    On,
    Off,
    Oneshot,
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

impl Command {
    pub fn execute<F, C>(
        self,
        client: &mut C,
        _config: &'static Config,
        mut request_work: F,
    ) -> Result<(), anyhow::Error>
    where
        C: MpdClient,
        F: FnMut(WorkRequest, &mut C),
    {
        match self {
            Command::Play { position: None } => client.play()?,
            Command::Play { position: Some(pos) } => client.play_pos(pos)?,
            Command::Pause => client.pause()?,
            Command::TogglePause => client.pause_toggle()?,
            Command::Unpause => client.unpause()?,
            Command::Stop => client.stop()?,
            Command::Volume { value: Some(value) } => client.volume(value.parse()?)?,
            Command::Volume { value: None } => println!("{}", client.get_status()?.volume.value()),
            Command::Next => client.next()?,
            Command::Prev => client.prev()?,
            Command::Repeat { value } => client.repeat((value).into())?,
            Command::Random { value } => client.random((value).into())?,
            Command::Single { value } => client.single((value).into())?,
            Command::Consume { value } => client.consume((value).into())?,
            Command::Seek { value } => client.seek_current(value.parse()?)?,
            Command::Clear => client.clear()?,
            Command::Add { file } => client.add(&file)?,
            Command::AddYt { url } => {
                request_work(WorkRequest::DownloadYoutube { url }, client);
            }
            Command::Outputs => println!("{}", serde_json::ser::to_string(&client.outputs()?)?),
            Command::Config => bail!("Cannot use config command here."),
            Command::Theme => bail!("Cannot use theme command here."),
            Command::Version => bail!("Cannot use version command here."),
            Command::ToggleOutput { id } => client.toggle_output(id)?,
            Command::EnableOutput { id } => client.enable_output(id)?,
            Command::DisableOutput { id } => client.disable_output(id)?,
            Command::Status => println!("{}", serde_json::ser::to_string(&client.get_status()?)?),
            Command::Song => println!("{}", serde_json::ser::to_string(&client.get_current_song()?)?),
            Command::Mount { ref name, ref path } => client.mount(name, path)?,
            Command::Unmount { ref name } => client.unmount(name)?,
            Command::ListMounts => println!("{}", serde_json::ser::to_string(&client.list_mounts()?)?),
        };
        Ok(())
    }
}

impl FromStr for Args {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Args::try_parse_from(std::iter::once("").chain(s.split_whitespace()))?)
    }
}

fn get_default_config_path() -> PathBuf {
    let mut path = PathBuf::new();
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        path.push(dir);
    } else if let Ok(home) = std::env::var("HOME") {
        path.push(home);
        path.push(".config");
    } else {
        return path;
    }
    path.push(env!("CARGO_CRATE_NAME"));
    #[cfg(debug_assertions)]
    path.push("config.debug.ron");
    #[cfg(not(debug_assertions))]
    path.push("config.ron");
    return path;
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum ImageMethodFile {
    Kitty,
    UeberzugWayland,
    UeberzugX11,
    None,
    #[default]
    Auto,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMethod {
    Kitty,
    UeberzugWayland,
    UeberzugX11,
    None,
    #[default]
    Unsupported,
}

#[derive(Debug, Default)]
pub struct Config {
    pub address: &'static str,
    pub cache_dir: Option<&'static str>,
    pub volume_step: u8,
    pub keybinds: KeyConfig,
    pub status_update_interval_ms: Option<u64>,
    pub theme: UiConfig,
    pub image_method: ImageMethod,
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
    #[serde(default)]
    keybinds: KeyConfigFile,
    #[serde(default)]
    image_method: ImageMethodFile,
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
            image_method: ImageMethodFile::Auto,
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

        let mut config = Config {
            image_method: ImageMethod::default(),
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
        };

        config.image_method = if config.theme.album_art_width_percent == 0 {
            ImageMethod::None
        } else {
            match self.image_method {
                ImageMethodFile::Kitty if crate::utils::image_proto::is_kitty_supported()? => ImageMethod::Kitty,
                ImageMethodFile::Kitty => ImageMethod::Unsupported,
                ImageMethodFile::UeberzugWayland if crate::utils::image_proto::is_ueberzug_wayland_supported() => {
                    ImageMethod::UeberzugWayland
                }
                ImageMethodFile::UeberzugWayland => ImageMethod::Unsupported,
                ImageMethodFile::UeberzugX11 if crate::utils::image_proto::is_ueberzug_x11_supported() => {
                    ImageMethod::UeberzugX11
                }
                ImageMethodFile::UeberzugX11 => ImageMethod::Unsupported,
                ImageMethodFile::None => ImageMethod::None,
                ImageMethodFile::Auto if config.theme.album_art_width_percent == 0 => ImageMethod::None,
                ImageMethodFile::Auto => match crate::utils::image_proto::determine_image_support()? {
                    ImageProtocol::Kitty => ImageMethod::Kitty,
                    ImageProtocol::UeberzugWayland => ImageMethod::UeberzugWayland,
                    ImageProtocol::UeberzugX11 => ImageMethod::UeberzugX11,
                    ImageProtocol::None => ImageMethod::None,
                },
            }
        };

        match config.image_method {
            ImageMethod::Unsupported => {
                status_warn!(
                    "Album art is enabled but no image protocol is supported by your terminal, disabling album art"
                );
                config.theme.album_art_width_percent = 0;
            }
            ImageMethod::None => {
                config.theme.album_art_width_percent = 0;
            }
            ImageMethod::Kitty | ImageMethod::UeberzugWayland | ImageMethod::UeberzugX11 => {
                log::debug!(requested:? = self.image_method, resolved:? = config.image_method; "Image method resolved");
            }
        }

        Ok(config)
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
