use std::{path::Path, time::Duration};

use anyhow::{Result, bail};
use crossbeam::channel::Sender;
use notify_debouncer_full::{
    DebounceEventResult,
    DebouncedEvent,
    Debouncer,
    RecommendedCache,
    new_debouncer,
    notify::{
        Event,
        EventKind,
        RecommendedWatcher,
        RecursiveMode,
        event::{AccessKind, AccessMode},
    },
};

use crate::shared::{events::WorkRequest, macros::try_skip};

#[must_use = "Returns a drop guard for the config directory watcher"]
pub(crate) fn init(
    lyrics_directory: &Path,
    request_tx: Sender<WorkRequest>,
) -> Result<Debouncer<RecommendedWatcher, RecommendedCache>> {
    if !lyrics_directory.exists() {
        bail!("Lyrics path {} does not exist", lyrics_directory.display());
    }

    let mut watcher = {
        let lyrics_directory = lyrics_directory.to_path_buf(); // owned value required for error logging

        new_debouncer(Duration::from_millis(500), None, move |event: DebounceEventResult| {
            let events = match event {
                Ok(events) => events,
                Err(err) => {
                    log::error!(err:?, lyrics_directory:?; "Encountered error while watching lyrics dir");
                    return;
                }
            };

            for event in events {
                let DebouncedEvent { event: Event { kind, paths, .. }, .. } = event;

                if !matches!(kind, EventKind::Access(AccessKind::Close(AccessMode::Write))) {
                    continue;
                }

                for path in paths {
                    if path.extension().is_none_or(|ext| ext != "lrc") {
                        continue;
                    }

                    log::debug!(path:?; "File event");

                    try_skip!(
                        request_tx.send(WorkRequest::IndexSingleLrc { path }),
                        "Failed to send lyrics changed event"
                    );
                }
            }
        })?
    };

    log::info!(lyrics_dir:? = lyrics_directory.to_str(); "Watching for changes");
    watcher.watch(lyrics_directory, RecursiveMode::Recursive)?;

    Ok(watcher)
}
