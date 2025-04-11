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

use crate::{AppEvent, config::ConfigFile, shared::macros::try_skip};

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

                let Ok(config) = ConfigFile::read(&config_path).inspect_err(|err| {
                    try_skip!(
                        event_tx.send(AppEvent::InfoModal {
                            message: vec![err.to_string()],
                            title: None,
                            size: None,
                        }),
                        "Failed to send info modal request"
                    );
                }) else {
                    continue;
                };

                let Ok(config) = config
                    .into_config(Some(&config_path), None, None, None, true)
                    .inspect_err(|err| {
                        try_skip!(
                            event_tx.send(AppEvent::InfoModal {
                                message: vec![err.to_string()],
                                title: None,
                                size: None,
                            }),
                            "Failed to send info modal request"
                        );
                    })
                else {
                    continue;
                };
                theme_name = config.theme_name.as_ref().map(|c| format!("{c}.ron"));

                try_skip!(
                    event_tx.send(AppEvent::ConfigChanged { config, keep_old_theme: false }),
                    "Failed to send config changed event"
                );
            }
        },
    )?;

    watcher.watch(&config_directory, RecursiveMode::Recursive)?;
    log::info!(config_directory:? = config_directory.to_str(); "Watching for changes");

    Ok(watcher)
}
