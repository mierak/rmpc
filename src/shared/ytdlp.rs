use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use anyhow::{Context, Result, anyhow, bail};
use itertools::Itertools;
use rustix::path::Arg;
use serde::Deserialize;
use walkdir::WalkDir;

use super::dependencies;
use crate::{
    config::cli_config::CliConfig,
    shared::macros::{status_error, status_info, status_warn},
};

#[derive(Debug)]
pub struct YtDlp<'a> {
    pub cache_dir: &'a Path,
}
#[derive(Debug, Deserialize)]
struct YtdlpSearchJson {
    entries: Vec<YtdlpSearchEntry>,
}

#[derive(Debug, Deserialize)]
struct YtdlpSearchEntry {
    id: String,
}

impl<'a> YtDlp<'a> {
    fn new(cache_dir: &'a Path) -> Result<Self> {
        if which::which("yt-dlp").is_err() {
            bail!("yt-dlp was not found on PATH. Please install yt-dlp and try again.")
        }

        std::fs::create_dir_all(cache_dir)?;
        Ok(Self { cache_dir })
    }

    pub fn init_and_download(config: &CliConfig, url: &str) -> Result<Vec<String>> {
        let Some(cache_dir) = &config.cache_dir else {
            bail!("Youtube support requires 'cache_dir' to be configured")
        };

        if let Err(unsupported_list) = dependencies::is_youtube_supported(&config.address) {
            status_warn!(
                "Youtube support requires the following and may thus not work properly: {}",
                unsupported_list.join(", ")
            );
        } else {
            status_info!("Downloading '{url}'");
        }

        let ytdlp = YtDlp::new(cache_dir)?;
        let file_path = ytdlp.download(url)?;

        Ok(file_path)
    }

    pub fn search_youtube_single(query: &str) -> Result<String> {
        let search_expr = format!("ytsearch1:{query}");

        let mut command = Command::new("yt-dlp");
        command.arg("-J");
        command.arg("--flat-playlist");
        command.arg(&search_expr);

        let args = command.get_args().map(|arg| format!("\"{}\"", arg.to_string_lossy())).join(" ");
        log::debug!(args = args.as_str(); "Executing yt-dlp search");

        let out = command.output()?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let exit_code = out.status.code();

        if exit_code != Some(0) {
            log::error!(stderr = stderr.as_str().trim(); "yt-dlp search failed");
            bail!(
                "yt-dlp search failed with exit code: {}",
                exit_code.map_or_else(|| "None".to_string(), |c| c.to_string())
            );
        }

        let parsed: YtdlpSearchJson =
            serde_json::from_str(&stdout).context("Failed to parse yt-dlp search JSON")?;

        let entry = parsed
            .entries
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No results for query: {query}"))?;

        let host = YtDlpHost {
            id: entry.id.clone(),
            filename: entry.id.clone(),
            kind: YtDlpHostKind::Youtube,
        };

        Ok(host.to_url())
    }

    pub fn download(&self, url: &str) -> Result<Vec<String>> {
        let id: YtDlpPlaylistOrHost = url.parse()?;
        match id {
            YtDlpPlaylistOrHost::Single(id) => Ok(vec![self.download_single(&id)?]),
            YtDlpPlaylistOrHost::Playlist(playlist) => {
                let mut command = Command::new("yt-dlp");
                command.arg("--print");
                command.arg("%(id)s");
                command.arg("--flat-playlist");
                command.arg("--compat-options");
                command.arg("no-youtube-unavailable-videos");
                command.arg(playlist.id);
                let args = command
                    .get_args()
                    .map(|arg| format!("\"{}\"", arg.to_string_lossy()))
                    .join(" ")
                    .to_string();
                log::debug!(args = args.as_str(); "Executing yt-dlp");

                let out = command.output()?;
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let exit_code = out.status.code();
                log::trace!(stdout = stdout.as_str().trim(), stderr = stderr.as_str().trim(), exit_code:?; "yt-dlp finished");

                if exit_code != Some(0) {
                    log::error!(stderr = stderr.as_str().trim();"yt-dlp failed");
                    bail!(
                        "yt-dlp failed with exit code: {}. Check logs for more details.",
                        exit_code.map_or_else(|| "None".to_string(), |c| c.to_string())
                    );
                }

                status_info!("Found {} videos in playlist", stdout.lines().count(),);

                let (success, error): (Vec<_>, Vec<_>) = stdout
                    .lines()
                    .map(|line| YtDlpHost {
                        id: line.to_owned(),
                        filename: line.to_owned(),
                        kind: playlist.kind,
                    })
                    .map(|id| self.download_single(&id))
                    .partition_result();

                for err in error {
                    status_error!("Failed to download video: {err}");
                }

                Ok(success)
            }
        }
    }

    fn download_single(&self, id: &YtDlpHost) -> Result<String> {
        if let Some(cached_file) = id.get_cached(self.cache_dir) {
            status_info!(file:? = cached_file.as_str(); "Youtube video id '{}' already downloaded", id.id);
            return Ok(cached_file.as_str()?.to_string());
        }

        status_info!("Downloading video with id: {}", id.id);

        let mut cache = id.cache_subdir(self.cache_dir);
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
            .to_string();
        log::debug!(args = args.as_str(); "Executing yt-dlp");

        let out = command.output()?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let exit_code = out.status.code();
        log::trace!(stdout = stdout.as_str().trim(), stderr = stderr.as_str().trim(), exit_code:?; "yt-dlp finished");

        if exit_code != Some(0) {
            log::error!(stderr = stderr.as_str().trim();"yt-dlp failed");
            if let Err(err) = id.delete_cached(self.cache_dir) {
                log::error!(err = err.to_string().as_str(); "Failed to cleanup after yt-dlp failed");
            }
            bail!(
                "yt-dlp failed with exit code: {}. Check logs for more details.",
                exit_code.map_or_else(|| "None".to_string(), |c| c.to_string())
            );
        }

        // yt-dlp for some reason does not respect output file template when
        // doing post processing with ffmpeg. This results in the file
        // having different extensions than the one specified so we work
        // around it by trying to find the file in the cache directory as that
        // should still be reliable.
        id.get_cached(self.cache_dir)
            .map(|v| -> Result<_> { Ok(v.as_str()?.to_string()) })
            .transpose()?
            .ok_or_else(|| anyhow!("Did not find file downloadid by yt-dlp in cache directory"))
    }
}

struct YtDlpPlaylist {
    kind: YtDlpHostKind,
    id: String,
}

enum YtDlpPlaylistOrHost {
    Single(YtDlpHost),
    Playlist(YtDlpPlaylist),
}

struct YtDlpHost {
    /// id of the video/audio, to be used in the url
    id: String,
    /// filename of the video/audio, will be used to cache the file
    filename: String,
    kind: YtDlpHostKind,
}

impl YtDlpHost {
    fn to_url(&self) -> String {
        match self.kind {
            YtDlpHostKind::Youtube => format!("https://www.youtube.com/watch?v={}", self.id),
            YtDlpHostKind::Soundcloud => format!("https://soundcloud.com/{}", self.id),
            YtDlpHostKind::NicoVideo => format!("https://www.nicovideo.jp/watch/{}", self.id),
        }
    }

    fn cache_subdir(&self, path: &Path) -> PathBuf {
        path.join(match self.kind {
            YtDlpHostKind::Youtube => "youtube",
            YtDlpHostKind::Soundcloud => "soundcloud",
            YtDlpHostKind::NicoVideo => "nicovideo",
        })
    }

    pub fn get_cached(&self, cache_dir: &Path) -> Option<PathBuf> {
        WalkDir::new(self.cache_subdir(Path::new(cache_dir)))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .find(|e| e.path().file_stem().is_some_and(|stem| stem == OsStr::new(&self.filename)))
            .map(|entry| entry.into_path())
    }

    pub fn delete_cached(&self, cache_dir: &Path) -> Result<()> {
        let files = WalkDir::new(self.cache_subdir(Path::new(cache_dir)))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().file_stem().is_some_and(|stem| stem == OsStr::new(&self.filename)))
            .map(|entry| entry.into_path());

        for file in files {
            log::debug!(file:? = file.as_str(); "Deleting cached file");
            std::fs::remove_file(file)?;
        }

        Ok(())
    }
}

#[derive(Clone, Copy)]
enum YtDlpHostKind {
    Youtube,
    Soundcloud,
    NicoVideo,
}

impl FromStr for YtDlpPlaylistOrHost {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let url = url::Url::parse(s)?;

        let Some(host) = url.host_str() else {
            bail!("Invalid yt-dlp url: '{}'. No hostname found.", s);
        };

        match host.strip_prefix("www.").unwrap_or(host) {
            "youtube.com" => {
                let segments = url
                    .path_segments()
                    .with_context(|| format!("Invalid youtube video url: '{s}'"))?
                    .collect_vec();

                let is_watch_url = segments.contains(&"watch");
                let is_playlist_url = segments.contains(&"playlist")
                    || url.query_pairs().any(|(key, _)| key == "list");

                if is_playlist_url {
                    url.query_pairs()
                        .find(|(k, _)| k == "list")
                        .map(|(_, v)| YtDlpPlaylist {
                            id: v.to_string(),
                            kind: YtDlpHostKind::Youtube,
                        })
                        .ok_or_else(|| anyhow!("No playlist id found in url"))
                        .map(YtDlpPlaylistOrHost::Playlist)
                } else if is_watch_url {
                    url.query_pairs()
                        .find(|(k, _)| k == "v")
                        .map(|(_, v)| YtDlpHost {
                            id: v.to_string(),
                            filename: v.to_string(),
                            kind: YtDlpHostKind::Youtube,
                        })
                        .ok_or_else(|| anyhow!("No video id found in url"))
                        .map(YtDlpPlaylistOrHost::Single)
                } else {
                    bail!("Invalid youtube video url: '{}'", s);
                }
            }
            "youtu.be" => url
                .path_segments()
                .with_context(|| format!("Invalid youtube video url: '{s}'"))?
                .next()
                .map(|x| YtDlpHost {
                    id: x.to_string(),
                    filename: x.to_string(),
                    kind: YtDlpHostKind::Youtube,
                })
                .ok_or_else(|| anyhow!("No video id found in url"))
                .map(YtDlpPlaylistOrHost::Single),
            "soundcloud.com" => {
                let mut path_segments = url.path_segments().context("cannot-be-a-base URL")?;
                let Some(username) = path_segments.next() else {
                    bail!("Invalid soundcloud url, no username: '{}'", s);
                };
                let Some(track_name) = path_segments.next() else {
                    bail!("Invalid soundcloud url, no track name: '{}'", s);
                };
                Ok(YtDlpPlaylistOrHost::Single(YtDlpHost {
                    id: format!("{username}/{track_name}"),
                    filename: format!("{username}-{track_name}"),
                    kind: YtDlpHostKind::Soundcloud,
                }))
            }
            "nicovideo.jp" => {
                let mut path_segments = url.path_segments().context("cannot-be-a-base URL")?;
                let Some(_watch_segment) = path_segments.next() else {
                    bail!("Invalid nicovideo url, no watch segment: '{}'", s);
                };
                let Some(id) = path_segments.next() else {
                    bail!("Invalid nicovideo url, no video id: '{}'", s);
                };
                Ok(YtDlpPlaylistOrHost::Single(YtDlpHost {
                    id: id.to_string(),
                    filename: id.to_string(),
                    kind: YtDlpHostKind::NicoVideo,
                }))
            }
            _ => bail!("Invalid yt-dlp url: '{}'. Received hostname: '{}'", s, host),
        }
    }
}
