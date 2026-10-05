use std::{path::PathBuf, sync::Arc, thread::JoinHandle, time::Duration};

use anyhow::{Context, Result, bail};
use crossbeam::channel::Sender;
use notify_debouncer_full::{
    DebounceEventResult,
    Debouncer,
    RecommendedCache,
    new_debouncer,
    notify::{
        EventKind,
        RecommendedWatcher,
        RecursiveMode,
        event::{AccessKind, AccessMode},
    },
};
use parking_lot::Mutex;
use rmpc_shared::paths::theme_paths;
use signal_hook::{
    consts::{SIGUSR1, SIGUSR2},
    iterator::Signals,
};

use crate::{
    AppEvent,
    config::{ConfigFile, theme::UiConfig},
    shared::{
        config_read::{
            ConfigReadError,
            find_first_existing_path,
            read_config_file,
            read_theme_file,
        },
        macros::try_skip,
    },
};

pub const ERROR_CONFIG_MODAL_ID: &str = "config_error_modal";

/// Re-reads the config and theme files and sends the result to the event loop.
/// Shared by the config directory watcher and the SIGUSR1/SIGUSR2 handler.
pub(crate) struct ConfigReloader {
    config_path: PathBuf,
    config_directory: PathBuf,
    theme_name: Option<PathBuf>,
    error_modal_shown: bool,
    event_tx: Sender<AppEvent>,
}

impl ConfigReloader {
    pub(crate) fn new(
        config_path: PathBuf,
        theme_name: Option<PathBuf>,
        event_tx: Sender<AppEvent>,
    ) -> Result<Arc<Mutex<Self>>> {
        if !config_path.exists() {
            bail!("Config path {} does not exist", config_path.display());
        }

        let config_directory = config_path
            .parent()
            .with_context(|| format!("Invalid config directory {}", config_path.display()))?
            .to_owned();

        Ok(Arc::new(Mutex::new(Self {
            config_path,
            config_directory,
            theme_name,
            error_modal_shown: false,
            event_tx,
        })))
    }

    /// Re-reads both the config and the theme file. Used by the config
    /// directory watcher.
    fn reload(&mut self) {
        let Some(config) = self.read_config() else {
            return;
        };
        let Some(theme) = self.read_theme(&config) else {
            return;
        };

        let config = match config.into_config(theme, None, None, true) {
            Ok(config) => config,
            Err(err) => {
                self.show_error("Error: Failed to convert config file", &err);
                return;
            }
        };

        self.pop_error_modal();
        log::debug!("Config changed, sending event");
        try_skip!(
            self.event_tx
                .send(AppEvent::ConfigChanged { config: Box::new(config), keep_old_theme: false }),
            "Failed to send config changed event"
        );
    }

    /// Re-reads only the config file and keeps the current theme, same as
    /// `rmpc remote set config`.
    fn reload_config(&mut self) {
        let Some(config) = self.read_config() else {
            return;
        };

        let config = match config.into_config(UiConfig::default(), None, None, true) {
            Ok(config) => config,
            Err(err) => {
                self.show_error("Error: Failed to convert config file", &err);
                return;
            }
        };

        self.pop_error_modal();
        log::debug!("Config changed, sending event");
        try_skip!(
            self.event_tx
                .send(AppEvent::ConfigChanged { config: Box::new(config), keep_old_theme: true }),
            "Failed to send config changed event"
        );
    }

    /// Re-reads only the theme file set in the config file, same as `rmpc
    /// remote set theme`.
    fn reload_theme(&mut self) {
        let Some(config) = self.read_config() else {
            return;
        };
        let Some(theme) = self.read_theme(&config) else {
            return;
        };

        self.pop_error_modal();
        log::debug!("Theme changed, sending event");
        try_skip!(
            self.event_tx.send(AppEvent::ThemeChanged { theme: Box::new(theme) }),
            "Failed to send theme changed event"
        );
    }

    fn read_config(&mut self) -> Option<ConfigFile> {
        read_config_file(&self.config_path)
            .inspect_err(|err| self.show_error("Error: Failed to read config file", err))
            .ok()
    }

    fn read_theme(&mut self, config: &ConfigFile) -> Option<UiConfig> {
        let (theme_path, theme) = match &config.theme {
            Some(theme_name) => {
                let theme_paths = theme_paths(None, &self.config_path, theme_name);
                let chosen_theme_path = find_first_existing_path(theme_paths);

                let result = if let Some(theme_path) = chosen_theme_path {
                    read_theme_file(&theme_path)
                        .and_then(|theme| {
                            UiConfig::try_from(theme).map_err(ConfigReadError::Conversion)
                        })
                        .map(|theme| (Some(theme_path), theme))
                } else {
                    Err(ConfigReadError::ThemeNotFound)
                };

                match result {
                    Ok((theme_path, theme)) => (theme_path, theme),
                    Err(err) => {
                        self.show_error("Error: Failed to read theme file", &err);
                        return None;
                    }
                }
            }
            // No theme set in the config file, this is OK, use the default theme
            None => (None, UiConfig::default()),
        };

        // Persist the current theme name for future file events to only
        // trigger when the currently active theme
        // changes
        if let Some(theme_path) = theme_path
            && let Ok(path) = theme_path.strip_prefix(&self.config_directory)
        {
            self.theme_name = Some(path.to_owned());
        } else {
            self.theme_name = None;
        }

        Some(theme)
    }

    fn pop_error_modal(&mut self) {
        if self.error_modal_shown {
            self.error_modal_shown = false;
            try_skip!(
                self.event_tx
                    .send(AppEvent::UiAppEvent(crate::ui::UiAppEvent::PopConfigErrorModal)),
                "Failed to pop config error modal"
            );
        }
    }

    fn show_error(&mut self, title: &str, err: &dyn std::fmt::Display) {
        self.error_modal_shown = true;
        try_skip!(
            self.event_tx.send(AppEvent::InfoModal {
                message: vec![title.to_string(), "Caused by:".to_string(), format!("  {err}")],
                replacement_id: Some(ERROR_CONFIG_MODAL_ID.into()),
                title: None,
                size: None,
            }),
            "Failed to send info modal request"
        );
    }
}

#[must_use = "Returns a drop guard for the config directory watcher"]
pub(crate) fn init(
    reloader: Arc<Mutex<ConfigReloader>>,
) -> Result<Debouncer<RecommendedWatcher, RecommendedCache>> {
    let (config_file_name, config_directory) = {
        let reloader = reloader.lock();
        let config_file_name = reloader
            .config_path
            .file_name()
            .with_context(|| format!("Invalid config path {}", reloader.config_path.display()))?
            .to_owned();
        (config_file_name, reloader.config_directory.clone())
    };

    let mut watcher = new_debouncer(
        Duration::from_millis(500),
        None,
        move |event: DebounceEventResult| {
            let events = match event {
                Ok(events) => events,
                Err(err) => {
                    log::error!(err:?, config_file_name:?; "Encountered error while watching config file");
                    return;
                }
            };

            for event in events {
                let mut reloader = reloader.lock();
                if !event.paths.iter().any(|path| {
                    path.ends_with(&config_file_name)
                        || reloader.theme_name.as_ref().is_some_and(|theme| path.ends_with(theme))
                }) {
                    continue;
                }
                if !matches!(event.kind, EventKind::Access(AccessKind::Close(AccessMode::Write))) {
                    continue;
                }

                log::debug!(event:?; "File event");
                reloader.reload();
            }
        },
    )?;

    watcher.watch(&config_directory, RecursiveMode::Recursive)?;
    log::info!(config_directory:? = config_directory.to_str(); "Watching for changes");

    Ok(watcher)
}

/// Reloads the config file on SIGUSR1 and the theme file on SIGUSR2.
#[must_use = "Returns a drop guard for the signal handler"]
pub(crate) fn init_signals(reloader: Arc<Mutex<ConfigReloader>>) -> Result<SignalGuard> {
    let mut signals =
        Signals::new([SIGUSR1, SIGUSR2]).context("Failed to register reload signal handler")?;
    let handle = signals.handle();

    let thread = std::thread::Builder::new().name("reload_signals".to_string()).spawn(move || {
        for signal in signals.forever() {
            match signal {
                SIGUSR1 => {
                    log::info!("Received SIGUSR1, reloading config");
                    reloader.lock().reload_config();
                }
                SIGUSR2 => {
                    log::info!("Received SIGUSR2, reloading theme");
                    reloader.lock().reload_theme();
                }
                _ => {}
            }
        }
    })?;

    Ok(SignalGuard { handle, thread: Some(thread) })
}

pub struct SignalGuard {
    handle: signal_hook::iterator::Handle,
    thread: Option<JoinHandle<()>>,
}

impl Drop for SignalGuard {
    fn drop(&mut self) {
        self.handle.close();
        if let Some(thread) = self.thread.take() {
            try_skip!(thread.join().map_err(|_| "panicked"), "Signal handler thread panicked");
        }
    }
}
