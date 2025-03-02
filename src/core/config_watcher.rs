use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result};
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

use crate::{AppEvent, config::ConfigFile, status_warn};

#[must_use = "Returns a drop guard for the config directory watcher"]
pub(crate) fn init(
    config_path: PathBuf,
    theme_name: Option<String>,
    event_tx: Sender<AppEvent>,
) -> Result<Debouncer<RecommendedWatcher, RecommendedCache>> {
    let config_file_name = config_path
        .file_name()
        .with_context(|| format!("Invalid config path {config_path:?}"))?
        .to_owned();
    let config_directory = config_path
        .parent()
        .with_context(|| format!("Invalid config directory {config_path:?}"))?
        .to_owned();

    let mut theme_name = theme_name;
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
                if !event.paths.iter().any(|path| {
                    path.ends_with(&config_file_name)
                        || theme_name.as_ref().is_some_and(|theme| path.ends_with(theme))
                }) {
                    continue;
                }
                if !matches!(event.kind, EventKind::Access(AccessKind::Close(AccessMode::Write))) {
                    continue;
                }

                log::debug!(event:?; "File event");
                if let Err(err) = ConfigFile::read(&config_path)
                    .and_then(|config| config.into_config(Some(&config_path), None, None, true))
                    .inspect(|config| {
                        theme_name = config.theme_name.as_ref().map(|c| format!("{c}.ron"));
                    })
                    .and_then(|config| Ok(event_tx.send(AppEvent::ConfigChanged { config })?))
                {
                    status_warn!(err:?, config_path:?; "Failed to read config. Keeping the current config. Check logs for more information");
                }
            }
        },
    )?;

    watcher.watch(&config_directory, RecursiveMode::Recursive)?;
    log::info!(config_directory:? = config_directory.to_str(); "Watching for changes");

    Ok(watcher)
}
