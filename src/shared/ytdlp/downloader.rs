use std::{path::PathBuf, process::Command};

use itertools::Itertools;

use crate::shared::ytdlp::{
    error::YtDlpDownloadError,
    ytdlp_item::{YtDlpHost, YtDlpItem, YtDlpPlaylist},
};

pub struct YtDlp {
    pub cache_dir: PathBuf,
}

#[derive(Debug)]
pub struct YtDlpDownloadResult {
    pub file_path: PathBuf,
    pub stderr: String,
    pub stdout: String,
    pub exit_code: Option<i32>,
    pub was_already_downloaded: bool,
}

impl YtDlpDownloadResult {
    pub fn new_already_downloaded(file_path: PathBuf) -> Self {
        Self {
            file_path,
            stderr: String::new(),
            stdout: String::new(),
            was_already_downloaded: true,
            exit_code: None,
        }
    }
}

#[derive(serde::Deserialize)]
struct SearchEntry {
    id: Option<String>,
    url: Option<String>,
    #[serde(default)]
    webpage_url: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(serde::Deserialize)]
struct SearchJson {
    entries: Vec<SearchEntry>,
}

#[derive(Debug, Clone)]
pub struct YtDlpSearchItem {
    pub title: Option<String>,
    pub url: String,
}

impl YtDlp {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    pub fn search(
        kind: YtDlpHost,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<YtDlpSearchItem>> {
        let expr = format!("{}{}:{query}", kind.search_key(), limit.max(1));
        let out =
            std::process::Command::new("yt-dlp").args(["-J", "--flat-playlist", &expr]).output()?;
        if !out.status.success() {
            anyhow::bail!("yt-dlp search failed: {}", String::from_utf8_lossy(&out.stderr));
        }
        let parsed: SearchJson = serde_json::from_slice(&out.stdout)?;
        let items = parsed
            .entries
            .into_iter()
            .filter_map(|e| {
                let url = match kind {
                    YtDlpHost::Soundcloud => e.webpage_url.or(e.url),
                    _ => e.url.or(e.webpage_url),
                }
                .or_else(|| e.id.clone().map(|id| kind.watch_url(&id)))?;
                Some(YtDlpSearchItem { title: e.title, url })
            })
            .collect::<Vec<_>>();

        Ok(items)
    }

    pub fn download_single(
        &self,
        id: &YtDlpItem,
    ) -> Result<YtDlpDownloadResult, YtDlpDownloadError> {
        if let Some(cached_file) = id.get_cached(&self.cache_dir) {
            return Ok(YtDlpDownloadResult::new_already_downloaded(cached_file));
        }

        let mut cache = id.cache_subdir(&self.cache_dir);
        std::fs::create_dir_all(&cache)?;
        cache.push(format!("{}.%(ext)s", id.filename));

        let mut command = Command::new("yt-dlp");
        command.arg("-x");
        command.arg("--embed-thumbnail");
        command.arg("--embed-metadata");
        command.arg("-f");
        command.arg("bestaudio");
        command.arg("--convert-thumbnails");
        command.arg("jpg");
        command.arg("--output");
        command.arg(cache);
        command.arg(id.to_url());
        let args = command
            .get_args()
            .map(|arg| format!("\"{}\"", arg.to_string_lossy()))
            .join(" ")
            .clone();
        log::debug!(args = args.as_str(); "Executing yt-dlp");

        let out = command.output()?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let exit_code = out.status.code();
        log::debug!(stdout = stdout.as_str().trim(), stderr = stderr.as_str().trim(), exit_code:?; "yt-dlp finished");

        if exit_code != Some(0) {
            log::error!(stderr = stderr.as_str().trim();"yt-dlp failed");
            if let Err(err) = id.delete_cached(&self.cache_dir) {
                log::error!(err = err.to_string().as_str(); "Failed to cleanup after yt-dlp failed");
            }

            return Err(YtDlpDownloadError::YtDlpError { stdout, stderr, code: exit_code });
        }

        // yt-dlp for some reason does not respect output file template when
        // doing post processing with ffmpeg. This results in the file
        // having different extensions than the one specified so we work
        // around it by trying to find the file in the cache directory as that
        // should still be reliable.
        match id.get_cached(&self.cache_dir) {
            Some(file_path) => Ok(YtDlpDownloadResult {
                file_path,
                stderr,
                stdout,
                was_already_downloaded: false,
                exit_code,
            }),
            None => Err(YtDlpDownloadError::FileNotFound { stdout, stderr, code: exit_code }),
        }
    }

    pub fn resolve_playlist_urls(
        &self,
        playlist: &YtDlpPlaylist,
    ) -> Result<Vec<YtDlpItem>, YtDlpDownloadError> {
        let mut command = Command::new("yt-dlp");
        command.arg("--print");
        command.arg("%(id)s");
        command.arg("--flat-playlist");
        command.arg("--compat-options");
        command.arg("no-youtube-unavailable-videos");
        command.arg(&playlist.id);
        let args = command
            .get_args()
            .map(|arg| format!("\"{}\"", arg.to_string_lossy()))
            .join(" ")
            .clone();
        log::debug!(args = args.as_str(); "Executing yt-dlp");

        let out = command.output()?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let exit_code = out.status.code();
        log::debug!(stdout = stdout.as_str().trim(), stderr = stderr.as_str().trim(), exit_code:?; "yt-dlp finished");

        if exit_code != Some(0) {
            log::error!(stderr = stderr.as_str().trim();"yt-dlp failed");
            return Err(YtDlpDownloadError::YtDlpError { stdout, stderr, code: exit_code });
        }

        Ok(stdout
            .lines()
            .map(|line| YtDlpItem {
                id: line.to_owned(),
                filename: line.to_owned(),
                kind: playlist.kind,
            })
            .collect())
    }
}
