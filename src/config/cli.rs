use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long, value_name = "FILE", default_value = get_default_config_path().into_os_string())]
    pub config: PathBuf,
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
    /// Scan MPD's music directory for updates.
    Update {
        /// If supplied, MPD will update only the provided directory/file. If not specified, everything is updated.
        path: Option<String>,
        /// Rmpc will wait for the update job to finish before returning.
        #[arg(short, long, default_value = "false")]
        wait: bool,
    },
    /// Scan MPD's music directory for updates. Also rescans unmodified files.
    Rescan {
        /// If supplied, MPD will update only the provided directory/file. If not specified, everything is updated.
        path: Option<String>,
        /// Rmpc will wait for the update job to finish before returning.
        #[arg(short, long, default_value = "false")]
        wait: bool,
    },
    /// Prints the default theme. Can be used to bootstrap your theme file.
    Theme,
    /// Saves the current album art to a file.
    /// Exit codes:
    ///   * 0: Success
    ///   * 1: Error
    ///   * 2: No album art found
    ///   * 3: No song playing
    AlbumArt {
        /// Output file where to save the album art, "-" for stdout
        #[arg(short, long)]
        output: String,
    },
    /// Prints information about optional runtime dependencies
    DebugInfo,
    /// Prints the rmpc version
    Version,
    /// Plays song at the position in the current playlist. Defaults to current paused song.
    Play {
        /// Index of the song in the queue
        position: Option<u32>,
    },
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
    /// Prints info about the current song.
    /// If --path specified, prints information about the song at the given path instead.
    /// If --path is specified multiple times, prints an array containing all the songs.
    Song {
        #[arg(short, long)]
        path: Option<Vec<String>>,
    },
    /// Mounts supported storage to MPD
    Mount { name: String, path: String },
    /// Unmounts storage with given name
    Unmount { name: String },
    /// List currently mounted storages
    ListMounts,
}

#[derive(Parser, ValueEnum, Copy, Clone, Debug, PartialEq)]
pub enum OnOff {
    /// Enable
    On,
    /// Disable
    Off,
}

#[derive(Parser, ValueEnum, Copy, Clone, Debug, PartialEq)]
pub enum OnOffOneshot {
    /// Enable
    On,
    /// Disable
    Off,
    /// Track get removed from playlist after it has been played
    Oneshot,
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
    path
}
