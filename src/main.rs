#![deny(clippy::unwrap_used, clippy::pedantic)]
#![allow(
    clippy::single_match,
    clippy::type_complexity,
    clippy::module_name_repetitions,
    clippy::unused_self,
    clippy::unnested_or_patterns,
    clippy::match_same_arms,
    clippy::manual_let_else,
    clippy::needless_return,
    clippy::zero_sized_map_values,
    clippy::too_many_lines,
    clippy::match_single_binding,
    clippy::struct_field_names,
    clippy::redundant_closure_for_method_calls,
    unused_macros
)]
use core::scheduler::Scheduler;
use std::io::{Read, Write};

use anyhow::{Context, Result};
use clap::Parser;
use config::{Leak, cli_config::CliConfigFile};
use context::AppContext;
use crossbeam::channel::unbounded;
use log::info;
use rustix::path::Arg;

use crate::{
    config::{
        ConfigFile,
        cli::{Args, Command},
    },
    mpd::client::Client,
    shared::{
        dependencies::{DEPENDENCIES, FFMPEG, FFPROBE, PYTHON3, PYTHON3MUTAGEN, UEBERZUGPP, YTDLP},
        env::ENV,
        events::{AppEvent, ClientRequest, WorkRequest},
        logging,
        macros::{status_warn, try_ret},
        mpd_query::{MpdCommand, MpdQuery, MpdQueryResult},
        tmux,
    },
};

#[cfg(test)]
mod tests {
    pub mod fixtures;
}

mod config;
mod context;
mod core;
mod mpd;
mod shared;
mod ui;

fn main() -> Result<()> {
    let mut args = Args::parse();
    match args.command {
        Some(Command::Config { current: false }) => {
            std::io::stdout().write_all(include_bytes!(
                "../docs/src/content/docs/next/assets/example_config.ron"
            ))?;
        }
        Some(Command::Theme { current: false }) => {
            std::io::stdout().write_all(include_bytes!(
                "../docs/src/content/docs/next/assets/example_theme.ron"
            ))?;
        }
        Some(Command::Config { current: true }) => {
            let mut file = std::fs::File::open(&args.config).with_context(|| {
                format!("Config file was not found at '{}'", args.config.to_string_lossy())
            })?;
            let mut config = String::new();
            file.read_to_string(&mut config)?;
            println!("{config}");
        }
        Some(Command::Theme { current: true }) => {
            let config_file = ConfigFile::read(&args.config).with_context(|| {
                format!("Config file was not found at '{}'", args.config.to_string_lossy())
            })?;
            let config_dir = args.config.parent().with_context(|| {
                format!("Invalid config path '{}'", args.config.to_string_lossy())
            })?;
            let theme_path = config_file
                .theme_path(config_dir)
                .context("No theme file specified in the config. Default theme is used.")?;
            let mut file = std::fs::File::open(&theme_path).with_context(|| {
                format!("Theme file was not found at '{}'", theme_path.to_string_lossy())
            })?;
            let mut theme = String::new();
            file.read_to_string(&mut theme)?;
            println!("{theme}");
        }
        Some(Command::DebugInfo) => {
            let config_file = ConfigFile::read(&args.config).unwrap_or_default();
            let config = config_file.clone().into_config(
                Some(&args.config),
                std::mem::take(&mut args.address),
                std::mem::take(&mut args.password),
                false,
            )?;
            let mut mpd_host = ENV.var("MPD_HOST").unwrap_or_else(|_| "unset".to_string());
            if let Some(at_idx) = mpd_host.find('@') {
                mpd_host.replace_range(..at_idx, "***");
            }
            let mpd_port = ENV.var("MPD_PORT").unwrap_or_else(|_| "unset".to_string());

            println!(
                "rmpc {}{}",
                env!("CARGO_PKG_VERSION"),
                option_env!("VERGEN_GIT_DESCRIBE").map(|g| format!(" git {g}")).unwrap_or_default()
            );
            println!("\n{:<20} {}", "Config path", args.config.as_str()?);
            println!("{:<20} {:?}", "Theme path", config_file.theme);

            println!("\nMPD:");
            println!("{:<20} {:?}", "Address", config_file.address);
            println!("{:<20} {:?}", "Resolved Address", config.address);
            println!("{:<20} {mpd_host}", "MPD_HOST");
            println!("{:<20} {mpd_port}", "MPD_PORT");

            println!("\nYoutube playback:");
            println!("{:<20} {:?}", "Cache dir", config.cache_dir);
            println!("{}", FFMPEG.display());
            println!("{}", FFPROBE.display());
            println!("{}", YTDLP.display());
            println!("{}", PYTHON3.display());
            println!("{}", PYTHON3MUTAGEN.display());

            println!("\nImage protocol:");
            println!("{:<20} {}", "Requested", config_file.album_art.method);
            println!("{:<20} {}", "Resolved", config.album_art.method);
            println!("{:<20} {}", "TMUX", tmux::is_inside_tmux());
            println!("{}", UEBERZUGPP.display());
        }
        Some(Command::Version) => {
            println!(
                "rmpc {}{}",
                env!("CARGO_PKG_VERSION"),
                option_env!("VERGEN_GIT_DESCRIBE").map(|g| format!(" git {g}")).unwrap_or_default()
            );
        }
        Some(cmd) => {
            logging::init_console().expect("Logger to initialize");
            let config: CliConfigFile = match CliConfigFile::read(&args.config) {
                Ok(cfg) => cfg,
                Err(_err) => ConfigFile::default().into(),
            };
            let config = config.into_config(args.address, args.password).leak();
            let mut client = Client::init(config.address, config.password, "main")?;
            client.set_read_timeout(None)?;
            (cmd.execute(config)?)(&mut client)?;
        }
        None => {
            let (worker_tx, worker_rx) = unbounded::<WorkRequest>();
            let (client_tx, client_rx) = unbounded::<ClientRequest>();
            let (event_tx, event_rx) = unbounded::<AppEvent>();
            logging::init(event_tx.clone()).expect("Logger to initialize");

            log::debug!(rev = env!("VERGEN_GIT_DESCRIBE"); "rmpc started");
            std::thread::Builder::new()
                .name("dependency_check".to_string())
                .spawn(|| DEPENDENCIES.iter().for_each(|d| d.log()))?;

            let config = match ConfigFile::read(&args.config) {
                Ok(val) => val.into_config(
                    Some(&args.config),
                    std::mem::take(&mut args.address),
                    std::mem::take(&mut args.password),
                    false,
                )?,
                Err(err) => {
                    status_warn!(err:?; "Failed to read config. Using default values. Check logs for more information");
                    ConfigFile::default().into_config(
                        None,
                        std::mem::take(&mut args.address),
                        std::mem::take(&mut args.password),
                        false,
                    )?
                }
            };

            if let Some(lyrics_dir) = config.lyrics_dir {
                try_ret!(
                    worker_tx.send(WorkRequest::IndexLyrics { lyrics_dir }),
                    "Failed to request lyrics indexing"
                );
            }
            try_ret!(event_tx.send(AppEvent::RequestRender), "Failed to render first frame");

            let mut client = try_ret!(
                Client::init(config.address, config.password, "command"),
                "Failed to connect to MPD"
            );
            client.set_read_timeout(Some(config.mpd_read_timeout))?;
            client.set_write_timeout(Some(config.mpd_write_timeout))?;

            let tx_clone = event_tx.clone();

            let context = try_ret!(
                AppContext::try_new(
                    &mut client,
                    config,
                    tx_clone,
                    worker_tx.clone(),
                    client_tx.clone(),
                    Scheduler::new((event_tx.clone(), client_tx.clone())),
                ),
                "Failed to create app context"
            );

            let enable_mouse = context.config.enable_mouse;
            let terminal = try_ret!(ui::setup_terminal(enable_mouse), "Failed to setup terminal");

            core::client::init(client_rx.clone(), event_tx.clone(), client, context.config)?;
            core::work::init(
                worker_rx.clone(),
                client_tx.clone(),
                event_tx.clone(),
                context.config,
            )?;
            core::input::init(event_tx.clone())?;
            let _sock_guard =
                core::socket::init(event_tx.clone(), worker_tx.clone(), context.config);
            let event_loop_handle = core::event_loop::init(context, event_rx, terminal)?;

            let original_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |panic| {
                crossterm::terminal::disable_raw_mode().expect("Disabling of raw mode to succeed");
                crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen)
                    .expect("Exit from alternate screen to succeed");
                original_hook(panic);
            }));

            info!("Application initialized successfully");

            let mut terminal = event_loop_handle.join().expect("event loop to not panic");
            try_ret!(
                ui::restore_terminal(&mut terminal, enable_mouse),
                "Terminal restore to succeed"
            );
        }
    }

    Ok(())
}
