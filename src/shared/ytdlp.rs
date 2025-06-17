use std::{
    io::ErrorKind,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use anyhow::{Context, Result, anyhow, bail};
use itertools::Itertools;
use rustix::path::Arg;

use super::dependencies;
use crate::{
    config::cli_config::CliConfig,
    shared::macros::{status_info, status_warn},
};

#[derive(Debug)]
pub struct YtDlp<'a> {
    pub cache_dir: &'a Path,
}

impl<'a> YtDlp<'a> {
    fn new(cache_dir: &'a Path) -> Result<Self> {
        if which::which("yt-dlp").is_err() {
            bail!("yt-dlp was not found on PATH. Please install yt-dlp and try again.")
        }

        std::fs::create_dir_all(cache_dir)?;
        Ok(Self { cache_dir })
    }

    pub fn init_and_download(config: &CliConfig, url: &str) -> Result<String> {
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

    pub fn download(&self, url: &str) -> Result<String> {
        let id: YtDlpHost = url.parse()?;

        if let Some(cached_file) = id.get_cached(self.cache_dir)? {
            log::debug!(file:? = cached_file.as_str(); "Youtube video already downloaded");
            return Ok(cached_file.as_str()?.to_string());
        }

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
        id.get_cached(self.cache_dir)?
            .map(|v| -> Result<_> { Ok(v.as_str()?.to_string()) })
            .transpose()?
            .ok_or_else(|| anyhow!("yt-dlp failed to download video"))
    }
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

    pub fn get_cached(&self, cache_dir: &Path) -> Result<Option<PathBuf>> {
        Ok(match std::fs::read_dir(self.cache_subdir(cache_dir)) {
            Ok(result) => {
                result.filter_map(std::result::Result::ok).map(|v| v.path()).find(|v| {
                    v.is_file()
                        && v.file_name().as_ref().is_some_and(|v| {
                            v.as_bytes()
                                .windows(self.filename.len())
                                // NOTE this will likely be a problem if we ever decide to support
                                // windows at some point
                                .any(|window| window == self.filename.as_bytes())
                        })
                })
            }
            Err(err) if matches!(err.kind(), ErrorKind::NotFound) => None,
            Err(err) => {
                Err(anyhow!("Encountered error when reading cached yt-dlp file. Error: {}", err))?
            }
        })
    }

    pub fn delete_cached(&self, cache_dir: &Path) -> Result<Vec<PathBuf>> {
        let files = match std::fs::read_dir(self.cache_subdir(cache_dir)) {
            Ok(result) => {
                result
                    .filter_map(std::result::Result::ok)
                    .map(|v| v.path())
                    .filter(|v| {
                        v.is_file()
                            && v.file_name().as_ref().is_some_and(|v| {
                                v.as_bytes()
                                    .windows(self.filename.len())
                                    // NOTE this will likely be a problem if we ever decide to
                                    // support windows at some point
                                    .any(|window| window == self.filename.as_bytes())
                            })
                    })
                    .collect()
            }
            Err(err) if matches!(err.kind(), ErrorKind::NotFound) => Vec::new(),
            Err(err) => {
                Err(anyhow!("Encountered error when deleting cached yt-dlp file. Error: {}", err))?
            }
        };

        for file in &files {
            std::fs::remove_file(file)?;
        }

        Ok(files)
    }
}

enum YtDlpHostKind {
    Youtube,
    Soundcloud,
    NicoVideo,
}

impl FromStr for YtDlpHost {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let url = url::Url::parse(s)?;

        let Some(host) = url.host_str() else {
            bail!("Invalid yt-dlp url: '{}'. No hostname found.", s);
        };

        match host.strip_prefix("www.").unwrap_or(host) {
            "youtube.com" => {
                let is_watch_url = url
                    .path_segments()
                    .with_context(|| format!("Invalid youtube video url: '{s}'"))?
                    .contains("watch");

                if !is_watch_url {
                    bail!("Invalid youtube video url: '{}'", s);
                }

                url.query_pairs()
                    .find(|(k, _)| k == "v")
                    .map(|(_, v)| YtDlpHost {
                        id: v.to_string(),
                        filename: v.to_string(),
                        kind: YtDlpHostKind::Youtube,
                    })
                    .ok_or_else(|| anyhow!("No video id found in url"))
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
                .ok_or_else(|| anyhow!("No video id foun in url")),
            "soundcloud.com" => {
                let mut path_segments = url.path_segments().context("cannot-be-a-base URL")?;
                let Some(username) = path_segments.next() else {
                    bail!("Invalid soundcloud url, no username: '{}'", s);
                };
                let Some(track_name) = path_segments.next() else {
                    bail!("Invalid soundcloud url, no track name: '{}'", s);
                };
                Ok(YtDlpHost {
                    id: format!("{username}/{track_name}"),
                    filename: format!("{username}-{track_name}"),
                    kind: YtDlpHostKind::Soundcloud,
                })
            }
            "nicovideo.jp" => {
                let mut path_segments = url.path_segments().context("cannot-be-a-base URL")?;
                let Some(_watch_segment) = path_segments.next() else {
                    bail!("Invalid nicovideo url, no watch segment: '{}'", s);
                };
                let Some(id) = path_segments.next() else {
                    bail!("Invalid nicovideo url, no video id: '{}'", s);
                };
                Ok(YtDlpHost {
                    id: id.to_string(),
                    filename: id.to_string(),
                    kind: YtDlpHostKind::NicoVideo,
                })
            }
            _ => bail!("Invalid yt-dlp url: '{}'. Received hostname: '{}'", s, host),
        }
    }
}
