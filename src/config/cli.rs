// NOTE: This file is also included from build.rs. crate:: may mean any of
// build.rs or main.rs, so remember to replicate the crate imports in build.rs,
// by using the `#[path = ""]` attribute for `mod`.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum, ValueHint};
use strum::IntoStaticStr;

use crate::mpd::QueuePosition;

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long, value_hint = ValueHint::AnyPath, value_name = "FILE")]
    pub config: Option<PathBuf>,
    #[arg(short, long, value_hint = ValueHint::AnyPath, value_name = "FILE")]
    pub theme: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<Command>,
    #[arg(short, long)]
    /// Override the address to connect to. Defaults to value in the config
    /// file.
    pub address: Option<String>,
    #[arg(short, long)]
    /// Override the MPD password
    pub password: Option<String>,

    #[command(flatten)]
    pub partition: Partition,
}

#[derive(Debug, clap::Args)]
#[group(required = false, multiple = true)]
pub struct Partition {
    /// Partition to connect to at startup
    #[clap(long)]
    pub partition: Option<String>,

    /// Automatically create the partition if it does not exist. Requires
    /// partition to be set.
    #[clap(long, requires = "partition")]
    pub autocreate: bool,
}

#[derive(ValueEnum, IntoStaticStr, strum::Display, Clone, Copy, Debug, PartialEq)]
#[clap(rename_all = "lower")]
pub enum AddRandom {
    Song,
    Artist,
    Album,
    AlbumArtist,
    Genre,
}

#[derive(Subcommand, Clone, Debug, PartialEq)]
#[clap(rename_all = "lower")]
pub enum Command {
    AddRandom {
        tag: AddRandom,
        count: usize,
    },
    /// Prints the default config. Can be used to bootstrap your config file.
    Config {
        /// If provided, print the current config instead of the default one.
        #[arg(short, long, default_value = "false")]
        current: bool,
    },
    /// Prints the default theme. Can be used to bootstrap your theme file.
    Theme {
        /// If provided, print the current theme instead of the default one.
        #[arg(short, long, default_value = "false")]
        current: bool,
    },
    /// Index the lyrics dir and display result, meant only for debugging
    /// purposes
    LyricsIndex,
    /// Scan MPD's music directory for updates.
    Update {
        /// If supplied, MPD will update only the provided directory/file. If
        /// not specified, everything is updated.
        path: Option<String>,
        /// Rmpc will wait for the update job to finish before returning.
        #[arg(short, long, default_value = "false")]
        wait: bool,
    },
    /// Scan MPD's music directory for updates. Also rescans unmodified files.
    Rescan {
        /// If supplied, MPD will update only the provided directory/file. If
        /// not specified, everything is updated.
        path: Option<String>,
        /// Rmpc will wait for the update job to finish before returning.
        #[arg(short, long, default_value = "false")]
        wait: bool,
    },
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
    /// Plays song at the position in the current playlist. Defaults to current
    /// paused song.
    Play {
        /// Index of the song in the queue
        position: Option<usize>,
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
    Prev {
        /// Go back to the start of the song if more than 5 seconds elapsed
        #[arg(
            short,
            long = "rewind-to-start",
            default_missing_value = "5",
            num_args = 0..=1
        )]
        rewind_to_start: Option<u64>,
    },
    /// Sets volume, relative if prefixed by + or -. Prints current volume if no
    /// arguments is given.
    Volume {
        #[arg(allow_negative_numbers(true))]
        value: Option<String>,
    },
    /// On or off
    Repeat {
        value: OnOff,
    },
    /// On or off
    Random {
        value: OnOff,
    },
    /// On, off or oneshot
    Single {
        value: OnOffOneshot,
    },
    /// On, off or oneshot
    Consume {
        value: OnOffOneshot,
    },
    /// Toggles the repeat mode
    ToggleRepeat,
    /// Toggles the random mode
    ToggleRandom,
    /// Toggles the single mode
    ToggleSingle {
        /// Skip the oneshot mode, i.e. toggle between on and off
        #[arg(short, long = "skip-oneshot")]
        skip_oneshot: bool,
    },
    /// Toggles the consume mode
    ToggleConsume {
        /// Skip the oneshot mode, i.e. toggle between on and off
        #[arg(short, long = "skip-oneshot")]
        skip_oneshot: bool,
    },
    /// Seeks current song(seconds), relative if prefixed by + or -
    Seek {
        #[arg(allow_negative_numbers(true))]
        value: String,
    },
    /// Clear the current queue
    Clear,
    /// Add a song to the current queue. Relative to music database root. '/' to
    /// add all files to the queue
    Add {
        /// Files to add to MPD's queue
        #[arg(value_hint = ValueHint::AnyPath)]
        files: Vec<PathBuf>,
        /// Rmpc checks whether MPD supports the added external file's
        /// extension and skips it if it does not. This option disables
        /// this behaviour and rmpc will try to add all the files
        #[arg(long = "skip-ext-check", default_value = "false")]
        skip_ext_check: bool,
        /// If provided, queue the new item at this position instead of the end
        /// of the queue. Allowed positions are <number> (absolute) and
        /// +<number> or -<number> (relative)
        #[arg(short, long, allow_negative_numbers = true)]
        position: Option<QueuePosition>,
    },
    /// Add a song from youtube to the current queue.
    AddYt {
        url: String,
        /// If provided, queue the new item at this position instead of the end
        /// of the queue. Allowed positions are <number> (absolute) and
        /// +<number> or -<number> (relative)
        #[arg(short, long, allow_negative_numbers = true)]
        position: Option<QueuePosition>,
    },
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
    /// List MPD decoder plugins
    Decoders,
    /// Prints various information like the playback status
    Status,
    /// Prints info about the current song.
    /// If --path specified, prints information about the song at the given path
    /// instead. If --path is specified multiple times, prints an array
    /// containing all the songs.
    Song {
        #[arg(short, long)]
        path: Option<Vec<String>>,
    },
    /// Mounts supported storage to MPD
    Mount {
        name: String,
        path: String,
    },
    /// Unmounts storage with given name
    Unmount {
        name: String,
    },
    /// List currently mounted storages
    ListMounts,
    /// List the currently existing partitions
    ListPartitions,
    /// Manipulate and query song stickers
    Sticker {
        #[command(subcommand)]
        cmd: StickerCmd,
    },
    /// Send a remote command to running rmpc instance
    Remote {
        /// PID of the rmpc instance to send the remote command to. If not
        /// provided, rmpc will try to notify all the running instances.
        #[arg(long)]
        pid: Option<u32>,
        #[command(subcommand)]
        command: RemoteCmd,
    },
}

#[derive(Subcommand, Clone, Debug, PartialEq)]
#[clap(rename_all = "lower")]
pub enum RemoteCmd {
    /// Notify rmpc that a new lyrics file has been added
    IndexLrc {
        /// Absolute path to the lrc file
        #[arg(short, long)]
        path: PathBuf,
    },
    /// Display a message in the status bar
    Status {
        /// Message to display in the status bar
        message: String,
        /// Controls the color of the message in the status bar
        #[arg(short, long)]
        #[clap(value_enum, default_value_t = Level::Info)]
        level: Level,
        /// How long should the message be displayed for in milliseconds
        #[arg(short, long = "timeout", default_value_t = 5000)]
        timeout: u64,
    },
    #[clap(hide = true)]
    Tmux { hook: String },
    /// Sets a value in running rmpc instance
    Set {
        #[command(subcommand)]
        command: SetCommand,
    },
}

#[derive(Subcommand, Clone, Debug, PartialEq)]
#[clap(rename_all = "lower")]
pub enum SetCommand {
    /// Replaces config in a running rmpc instance with the provided one, theme
    /// is NOT replaced.
    #[clap(hide = true)]
    Config {
        /// Value to set the path to. Can be either path to a file or "-" to
        /// read from stdin
        #[arg(value_hint = ValueHint::AnyPath)]
        path: String,
    },
    /// Replaces theme in a running rmpc instance with the provided one
    Theme {
        /// Value to set the path to. Can be either path to a file or "-" to
        /// read from stdin
        #[arg(value_hint = ValueHint::AnyPath)]
        path: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Parser, ValueEnum)]
pub enum Level {
    Info,
    Error,
    Warn,
}

#[derive(Subcommand, Clone, Debug, PartialEq)]
#[clap(rename_all = "lower")]
pub enum StickerCmd {
    /// Set sticker value for a song
    Set {
        /// Path to a song, relative to music directory root
        uri: String,
        /// Sticker key to set
        key: String,
        /// Sticker value that will be written
        value: String,
    },
    /// Get sticker value for a song
    Get {
        /// Path to a song, relative to music directory root
        uri: String,
        /// Sticker key to get
        key: String,
    },
    /// List all stickers of a song
    List {
        /// Path to a song, relative to music directory root
        uri: String,
    },
    /// Find all stickers of given name in  the specified directory
    Find {
        /// Path to a directory, relative to music directory root
        uri: String,
        /// Sticker key to search for
        key: String,
    },
    /// Delete a sticker from a song
    Delete {
        /// Path to a song, relative to music directory root
        uri: String,
        /// Sticker key to search delete
        key: String,
    },
    /// Delete all stickers in a song
    DeleteAll {
        /// Path to a song, relative to music directory root
        uri: String,
    },
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

impl Args {
    #[must_use]
    pub fn config_path(&self) -> PathBuf {
        if let Some(path) = &self.config {
            return path.to_owned();
        }
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
}
