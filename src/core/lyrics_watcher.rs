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

use crate::{AppEvent, shared::macros::try_skip};

#[must_use = "Returns a drop guard for the config directory watcher"]
pub(crate) fn init(
    lyrics_path: PathBuf,
    event_tx: Sender<AppEvent>,
) -> Result<Debouncer<RecommendedWatcher, RecommendedCache>> {
    if !lyrics_path.exists() {
        bail!("Lyrics path {} does not exist", lyrics_path.display());
    }

    let lyrics_file_name = lyrics_path
        .file_name()
        .with_context(|| format!("Invalid config path {}", lyrics_path.display()))?
        .to_owned();
    let lyrics_directory = lyrics_path
        .parent()
        .with_context(|| format!("Invalid config directory {}", lyrics_path.display()))?
        .to_owned();

    let mut watcher = new_debouncer(
        Duration::from_millis(500),
        None,
        move |event: DebounceEventResult| {
            let events = match event {
                Ok(events) => events,
                Err(err) => {
                    log::error!(err:?, lyrics_file_name:?; "Encountered error while watching lyrics file");
                    return;
                }
            };

            for event in events {
                if !event.paths.iter().any(|path| path.ends_with(&lyrics_file_name)) {
                    continue;
                }
                if !matches!(event.kind, EventKind::Access(AccessKind::Close(AccessMode::Write))) {
                    continue;
                }

                log::debug!(event:?; "File event");

                try_skip!(
                    event_tx.send(AppEvent::LyricsChanged),
                    "Failed to send config changed event"
                );
            }
        },
    )?;

    watcher.watch(&lyrics_directory, RecursiveMode::Recursive)?;
    log::info!(lyrics_directory:? = lyrics_directory.to_str(); "Watching for changes");

    Ok(watcher)
}
