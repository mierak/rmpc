use std::{cell::RefCell, collections::BTreeMap, path::PathBuf, str::FromStr};

use anyhow::{Result, bail};
use crossbeam::channel::Sender;
use rmpc_mpd::queue_position::QueuePosition;

use crate::shared::{
    events::WorkRequest,
    id::{self, Id},
    macros::{status_error, status_info},
    ytdlp::{
        YtDlpDownloadError,
        YtDlpDownloadResult,
        error::YtDlpParseError,
        ytdlp_item::{YtDlpContent, YtDlpItem},
    },
};

#[derive(Debug, derive_more::Deref, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct DownloadId(Id);

impl std::fmt::Display for DownloadId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl DownloadId {
    pub fn new() -> Self {
        Self(id::new())
    }
}

#[derive(Debug)]
pub struct YtDlpManager {
    queue: RefCell<BTreeMap<DownloadId, QueuedYtDlpItem>>,
    work_sender: Sender<WorkRequest>,
}

#[derive(Debug, Clone)]
pub struct QueuedYtDlpItem {
    pub state: DownloadState,
    pub add_position: Option<QueuePosition>,
    pub inner: YtDlpItem,
}

#[derive(Debug, Clone, strum::AsRefStr, strum::EnumDiscriminants, strum::Display)]
#[strum_discriminants(derive(strum::AsRefStr))]
pub enum DownloadState {
    Queued,
    Downloading,
    Completed { logs: Vec<String>, path: PathBuf },
    AlreadyDownloaded { path: PathBuf },
    Failed { logs: Vec<String> },
    Canceled,
}

impl YtDlpManager {
    pub fn new(work_sender: Sender<WorkRequest>) -> Self {
        Self { queue: RefCell::new(BTreeMap::new()), work_sender }
    }

    pub fn len(&self) -> usize {
        self.queue.borrow().len()
    }

    pub fn ids(&self) -> Vec<DownloadId> {
        self.queue.borrow().keys().copied().collect()
    }

    pub fn get(&self, id: DownloadId) -> Option<QueuedYtDlpItem> {
        self.queue.borrow().get(&id).cloned()
    }

    pub fn map_values<F, T>(&self, f: F) -> Vec<T>
    where
        F: FnMut(&QueuedYtDlpItem) -> T,
    {
        self.queue.borrow().values().map(f).collect()
    }

    pub fn download_url(
        &self,
        url: &str,
        position: Option<QueuePosition>,
    ) -> Result<(), YtDlpParseError> {
        let resolved = YtDlpContent::from_str(url)?;

        match resolved {
            YtDlpContent::Single(host) => {
                self.queue_download(host, position);
                self.download_next();
            }
            YtDlpContent::Playlist(playlist) => {
                if let Err(err) =
                    self.work_sender.send(WorkRequest::YtDlpResolvePlaylist { playlist })
                {
                    status_error!(err:?; "Failed to send playlist download request");
                } else {
                    status_info!("Fetching playlist info");
                }
            }
        }

        Ok(())
    }

    pub fn download_next(&self) {
        for (id, item) in self.queue.borrow_mut().iter_mut() {
            match item.state {
                // First queued item found, start downloading
                DownloadState::Queued => {
                    if let Err(err) = self
                        .work_sender
                        .send(WorkRequest::YtDlpDownload { id: *id, url: item.inner.clone() })
                    {
                        status_error!(err:?; "Failed to send download request");
                        break;
                    }

                    status_info!("Downloading {} from {}", item.inner.id, item.inner.kind);
                    item.state = DownloadState::Downloading;

                    // Only ever download one at a time
                    break;
                }
                // A different item is already downloading, do nothing
                DownloadState::Downloading => {
                    break;
                }
                // Noop, nothing to do with terminal states
                DownloadState::Completed { .. } => {}
                DownloadState::AlreadyDownloaded { .. } => {}
                DownloadState::Failed { .. } => {}
                DownloadState::Canceled => {}
            }
        }
    }

    pub fn redownload(&self, id: DownloadId) {
        if let Some(item) = self.queue.borrow_mut().get_mut(&id) {
            item.state = DownloadState::Queued;
        }
        self.download_next();
    }

    pub fn cancel_download(&self, id: DownloadId) {
        if let Some(item) = self.queue.borrow_mut().get_mut(&id) {
            item.state = DownloadState::Canceled;
        }
    }

    pub fn queue_download(&self, item: YtDlpItem, position: Option<QueuePosition>) {
        self.queue.borrow_mut().insert(DownloadId::new(), QueuedYtDlpItem {
            state: DownloadState::Queued,
            add_position: position,
            inner: item,
        });
    }

    pub fn queue_download_many(&self, items: Vec<YtDlpItem>) {
        status_info!("Queueing {} items for download", items.len());
        for item in items {
            self.queue_download(item, None);
        }
    }

    pub fn resolve_download(
        &self,
        id: DownloadId,
        result: Result<YtDlpDownloadResult, YtDlpDownloadError>,
    ) -> Result<(PathBuf, Option<QueuePosition>)> {
        if let Some(item) = self.queue.borrow_mut().get_mut(&id) {
            match result {
                Ok(result) => {
                    if result.was_already_downloaded {
                        item.state =
                            DownloadState::AlreadyDownloaded { path: result.file_path.clone() };
                        status_info!(
                            "File for {} was already downloaded, skipping download",
                            item.inner.id
                        );
                    } else {
                        item.state = DownloadState::Completed {
                            logs: Self::join_stdout_stderr(
                                &result.stdout,
                                &result.stderr,
                                result.exit_code,
                            ),
                            path: result.file_path.clone(),
                        };
                        status_info!("Downloaded {}", item.inner.id);
                    }
                    Ok((result.file_path, item.add_position))
                }
                Err(YtDlpDownloadError::YtDlpError { stdout, stderr, code }) => {
                    item.state = DownloadState::Failed {
                        logs: Self::join_stdout_stderr(&stdout, &stderr, code),
                    };
                    bail!("Download failed");
                }
                Err(YtDlpDownloadError::FileNotFound { stdout, stderr, code }) => {
                    item.state = DownloadState::Failed {
                        logs: Self::join_stdout_stderr(&stdout, &stderr, code),
                    };
                    bail!("Download failed because the downloaded file was not found");
                }
                Err(YtDlpDownloadError::IoError(err)) => {
                    item.state = DownloadState::Failed { logs: vec![err.to_string()] };
                    bail!("Download failed because of IO error");
                }
                Err(YtDlpDownloadError::InvalidConfig(err)) => {
                    item.state = DownloadState::Failed { logs: vec![err.to_string()] };
                    bail!(err);
                }
            }
        } else {
            Err(anyhow::anyhow!("Download ID not found"))
        }
    }

    fn join_stdout_stderr(stdout: &str, stderr: &str, exit_code: Option<i32>) -> Vec<String> {
        let mut logs = Vec::new();
        if stdout.is_empty() && stderr.is_empty() {
            logs.push("<no output>".to_string());
            return logs;
        }
        if !stdout.is_empty() {
            logs.extend(stdout.lines().map(|line| line.to_string()));
        }

        if !stderr.is_empty() {
            logs.push(String::from("\n")); // separate stdout and stderr
            logs.extend(stderr.lines().map(|line| line.to_string()));
        }

        if let Some(code) = exit_code {
            logs.push(String::from("\n"));
            logs.push(format!("yt-dlp exited with code: {code}"));
        }

        logs
    }
}
