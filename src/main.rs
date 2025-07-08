use core::scheduler::Scheduler;
use std::{
    fs::File,
    io::{Read, Write},
    sync::Arc,
};

use anyhow::{Context, Result};
use clap::Parser;
use config::{DeserError, cli_config::CliConfigFile};
use crossbeam::channel::unbounded;
use ctx::Ctx;
use log::info;
use rustix::path::Arg;
use shared::{
    dependencies::CAVA,
    ipc::{get_socket_path, list_all_socket_paths},
    macros::{status_warn, try_skip},
};

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
        mpd_query::{MpdCommand, MpdQuery, MpdQueryResult},
        tmux,
    },
};

#[cfg(test)]
mod tests {
    mod cli_integration;
    pub mod fixtures;
    mod remote_ipc;
}

mod config;
mod core;
mod ctx;
mod mpd;
mod shared;
mod ui;

fn main() -> Result<()> {
    let mut args = Args::parse();
    let config_path = args.config_path();
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
        Some(Command::Config { current: true }) => match File::open(&config_path) {
            Ok(mut file) => {
                let mut config = String::new();
                file.read_to_string(&mut config)?;
                println!("{config}");
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                eprintln!(
                    "Config file not found at '{}'. Use 'rmpc config' to see the default config.",
                    config_path.display()
                );
                std::process::exit(1);
            }
            Err(err) => return Err(err.into()),
        },
        Some(Command::Theme { current: true }) => {
            let config_file = match ConfigFile::read(&config_path) {
                Ok(config) => config,
                Err(DeserError::NotFound(_)) => {
                    eprintln!(
                        "Config file not found at '{}'. No theme file specified. Use 'rmpc theme' to see the default theme.",
                        config_path.display()
                    );
                    std::process::exit(1);
                }
                Err(err) => return Err(err.into()),
            };
            let config_dir = config_path.parent().with_context(|| {
                format!("Invalid config path '{}'", config_path.to_string_lossy())
            })?;
            let theme_path = config_file
                .theme_path(config_dir)
                .context("No theme file specified in the config. Default theme is used.")?;
            let mut file = File::open(&theme_path).with_context(|| {
                format!("Theme file was not found at '{}'", theme_path.to_string_lossy())
            })?;
            let mut theme = String::new();
            file.read_to_string(&mut theme)?;
            println!("{theme}");
        }
        Some(Command::DebugInfo) => {
            let config_file = ConfigFile::read(&config_path)
                .context("Failed to read config file")
                .unwrap_or_default();
            let config = config_file.clone().into_config(
                Some(&config_path),
                args.theme.as_deref(),
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
            println!("\n{:<20} {}", "Config path", config_path.as_str()?);
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

            println!("\nVisualizer:");
            println!("{}", CAVA.display());
        }
        Some(Command::Version) => {
            println!(
                "rmpc {}{}",
                env!("CARGO_PKG_VERSION"),
                option_env!("VERGEN_GIT_DESCRIBE").map(|g| format!(" git {g}")).unwrap_or_default()
            );
        }
        Some(Command::Remote { command, pid }) => {
            if let Some(pid) = pid {
                let path = get_socket_path(pid);
                command.write_to_socket(&path)?;
                eprintln!("Successfully sent remote command to {}", path.display());
            } else {
                for path in list_all_socket_paths()? {
                    if let Err(err) = command.clone().write_to_socket(&path) {
                        eprintln!("Failed to send remote command. Error: '{err:?}'");
                        continue;
                    }
                    eprintln!("Successfully sent remote command to {}", path.display());
                }
            }
        }
        Some(cmd) => {
            logging::init_console().expect("Logger to initialize");
            let config: CliConfigFile = match CliConfigFile::read(&config_path) {
                Ok(cfg) => cfg,
                Err(err) => {
                    log::warn!(
                        "Failed to read config file at '{}': {}. Using default values.",
                        config_path.display(),
                        err
                    );
                    ConfigFile::default().into()
                }
            };
            let config = config.into_config(args.address, args.password);
            let result = cmd.execute(&config)?;
            let mut client = Client::init(
                config.address.clone(),
                config.password.clone(),
                "main",
                args.partition.partition,
                args.partition.autocreate,
            )?;
            client.set_read_timeout(None)?;
            result(&mut client)?;
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

            let config = match ConfigFile::read(&config_path).and_then(|val| {
                val.into_config(
                    Some(&config_path),
                    args.theme.as_deref(),
                    std::mem::take(&mut args.address),
                    std::mem::take(&mut args.password),
                    false,
                )
            }) {
                Ok(cfg) => cfg,
                Err(DeserError::NotFound(err)) => {
                    status_warn!(err:?; "No config or theme file was found. Using default values.");
                    ConfigFile::default().into_config(
                        None,
                        None,
                        std::mem::take(&mut args.address),
                        std::mem::take(&mut args.password),
                        false,
                    )?
                }
                Err(err) => {
                    try_skip!(
                        event_tx.send(AppEvent::InfoModal {
                            message: vec![err.to_string()],
                            id: Some("config_error_modal".into()),
                            title: None,
                            size: None
                        }),
                        "Failed to send info modal request"
                    );
                    ConfigFile::default().into_config(
                        None,
                        None,
                        std::mem::take(&mut args.address),
                        std::mem::take(&mut args.password),
                        false,
                    )?
                }
            };

            config.validate()?;

            if let Some(lyrics_dir) = &config.lyrics_dir {
                worker_tx
                    .send(WorkRequest::IndexLyrics { lyrics_dir: lyrics_dir.clone() })
                    .context("Failed to request lyrics indexing")?;
            }
            event_tx.send(AppEvent::RequestRender).context("Failed to render first frame")?;

            let mut client = Client::init(
                config.address.clone(),
                config.password.clone(),
                "command",
                args.partition.partition,
                args.partition.autocreate,
            )
            .context("Failed to connect to MPD")?;
            client.set_read_timeout(Some(config.mpd_read_timeout))?;
            client.set_write_timeout(Some(config.mpd_write_timeout))?;

            let tx_clone = event_tx.clone();

            let ctx = Ctx::try_new(
                &mut client,
                config,
                tx_clone,
                worker_tx.clone(),
                client_tx.clone(),
                Scheduler::new((event_tx.clone(), client_tx.clone())),
            )
            .context("Failed to create app context")?;

            core::client::init(
                client_rx.clone(),
                event_tx.clone(),
                client,
                Arc::clone(&ctx.config),
            )?;
            core::work::init(
                worker_rx.clone(),
                client_tx.clone(),
                event_tx.clone(),
                Arc::clone(&ctx.config),
            )?;
            core::input::init(event_tx.clone())?;
            let _sock_guard =
                core::socket::init(event_tx.clone(), worker_tx.clone(), Arc::clone(&ctx.config))
                    .context("Failed to initialize socket listener")?;

            let _config_watcher_guard = ctx.config.enable_config_hot_reload.then_some(
                core::config_watcher::init(
                    config_path,
                    ctx.config.theme_name.as_ref().map(|n| format!("{n}.ron",)),
                    event_tx.clone(),
                )
                .inspect_err(|e| log::warn!("Failed to initialize config watcher: {e}")),
            );

            let enable_mouse = ctx.config.enable_mouse;
            let terminal =
                shared::terminal::setup(enable_mouse).context("Failed to setup terminal")?;

            let event_loop_handle = core::event_loop::init(ctx, event_rx, terminal)?;

            let original_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |panic| {
                crossterm::terminal::disable_raw_mode().expect("Disabling of raw mode to succeed");
                crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen)
                    .expect("Exit from alternate screen to succeed");
                original_hook(panic);
            }));

            info!("Application initialized successfully");

            let mut terminal = event_loop_handle.join().expect("event loop to not panic");

            shared::terminal::restore(&mut terminal, enable_mouse)
                .context("Terminal restore to succeed")?;
        }
    }

    Ok(())
}
