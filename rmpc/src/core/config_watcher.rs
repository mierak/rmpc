use std::{path::PathBuf, time::Duration};

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

use crate::{
    AppEvent,
    config::theme::UiConfig,
    shared::{
        config_read::{
            ConfigReadError,
            find_first_existing_path,
            read_config_file,
            read_theme_file,
        },
        macros::try_skip,
        paths::theme_paths,
    },
};

pub const ERROR_CONFIG_MODAL_ID: &str = "config_error_modal";

#[must_use = "Returns a drop guard for the config directory watcher"]
pub(crate) fn init(
    config_path: PathBuf,
    theme_name: Option<PathBuf>,
    event_tx: Sender<AppEvent>,
) -> Result<Debouncer<RecommendedWatcher, RecommendedCache>> {
    if !config_path.exists() {
        bail!("Config path {} does not exist", config_path.display());
    }

    let config_file_name = config_path
        .file_name()
        .with_context(|| format!("Invalid config path {}", config_path.display()))?
        .to_owned();
    let config_directory = config_path
        .parent()
        .with_context(|| format!("Invalid config directory {}", config_path.display()))?
        .to_owned();
    let config_directory2 = config_directory.clone();

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

                let config = match read_config_file(&config_path) {
                    Ok(cfg) => cfg,
                    Err(err) => {
                        try_skip!(
                            event_tx.send(AppEvent::InfoModal {
                                message: vec![
                                    "Error: Failed to read config file".to_string(),
                                    "Caused by:".to_string(),
                                    format!("  {err}"),
                                ],
                                replacement_id: Some(ERROR_CONFIG_MODAL_ID.into()),
                                title: None,
                                size: None,
                            }),
                            "Failed to send info modal request"
                        );
                        continue;
                    }
                };

                let (theme_path, theme) = match &config.theme {
                    Some(theme_name) => {
                        let theme_paths = theme_paths(None, &config_path, theme_name);
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
                                try_skip!(
                                    event_tx.send(AppEvent::InfoModal {
                                        message: vec![
                                            "Error: Failed to read theme file".to_string(),
                                            "Caused by:".to_string(),
                                            format!("  {err}"),
                                        ],
                                        replacement_id: Some(ERROR_CONFIG_MODAL_ID.into()),
                                        title: None,
                                        size: None,
                                    }),
                                    "Failed to send info modal request"
                                );
                                continue;
                            }
                        }
                    }
                    // No theme set in the config file, this is OK, use the default theme
                    None => (None, UiConfig::default()),
                };

                let Ok(config) = config.into_config(theme, None, None, true).inspect_err(|err| {
                    try_skip!(
                        event_tx.send(AppEvent::InfoModal {
                            message: vec![
                                "Error: Failed to convert config file".to_string(),
                                "Caused by:".to_string(),
                                format!("  {err}"),
                            ],
                            replacement_id: Some(ERROR_CONFIG_MODAL_ID.into()),
                            title: None,
                            size: None,
                        }),
                        "Failed to send info modal request"
                    );
                }) else {
                    continue;
                };

                // Persist the current theme name for future file events to only trigger when
                // the currently active theme changes
                if let Some(theme_path) = theme_path
                    && let Ok(path) = theme_path.strip_prefix(&config_directory2)
                {
                    theme_name = Some(path.to_owned());
                } else {
                    theme_name = None;
                }

                try_skip!(
                    event_tx.send(AppEvent::UiEvent(crate::ui::UiAppEvent::PopConfigErrorModal)),
                    "Failed to pop config error modal"
                );

                try_skip!(
                    event_tx.send(AppEvent::ConfigChanged {
                        config: Box::new(config),
                        keep_old_theme: false
                    }),
                    "Failed to send config changed event"
                );
            }
        },
    )?;

    watcher.watch(&config_directory, RecursiveMode::Recursive)?;
    log::info!(config_directory:? = config_directory.to_str(); "Watching for changes");

    Ok(watcher)
}
