use anyhow::{anyhow, bail, Result};
use itertools::Itertools;
use rustix::path::Arg;
use std::{os::unix::ffi::OsStrExt, path::PathBuf, process::Command, str::FromStr};

#[derive(Debug)]
pub struct YtDlp {
    pub cache_dir: String,
}

impl YtDlp {
    pub fn new(cache_dir: &'static str) -> Result<Self> {
        let cache_dir = format!("{cache_dir}youtube/");

        if which::which("yt-dlp").is_err() {
            bail!("yt-dlp was not found on PATH. Please install yt-dlp and try again.")
        }

        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    pub fn download(&self, url: &str) -> Result<String> {
        let id: VideoId = url.parse()?;

        if let Some(cached_file) = id.get_cached(&self.cache_dir)? {
            log::debug!(file:? = cached_file.as_str(); "Youtube video already downloaded");
            return Ok(cached_file.as_str()?.to_string());
        }

        let output = format!("{}%(id)s.%(ext)s", self.cache_dir);

        let mut command = Command::new("yt-dlp");
        command.args([
            "-x",
            "--embed-thumbnail",
            "--embed-metadata",
            "-f",
            "bestaudio",
            "--convert-thumbnails",
            "jpg",
            "--output",
            &output,
            &format!("https://www.youtube.com/watch?v={}", id.0),
        ]);
        let args = command
            .get_args()
            .map(|arg| format!("\"{}\"", arg.to_string_lossy()))
            .join(" ")
            .to_string();
        log::debug!(args = args.as_str(); "Executing yt-dlp");

        let out = command.output()?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stdout).to_string();
        let exit_code = out.status.code();
        log::debug!(stdout = stdout.as_str(), stderr = stderr.as_str(), exit_code:?; "yt-dlp finished");

        if exit_code != Some(0) {
            bail!("yt-dlp failed with exit code: {:?}", exit_code);
        }

        // yt-dlp for some reason does not respect output file template when doing post processing
        // with ffmpeg. This results in the file having different extensions than the one specified
        // so we work around it by trying to find the file in the cache directory as that should
        // still be reliable.
        id.get_cached(&self.cache_dir)?
            .map(|v| -> Result<_> { Ok(v.as_str()?.to_string()) })
            .transpose()?
            .ok_or_else(|| anyhow!("yt-dlp failed to download video"))
    }
}

struct VideoId(String);

impl VideoId {
    pub fn get_cached(&self, cache_dir: &str) -> Result<Option<PathBuf>> {
        Ok(std::fs::read_dir(cache_dir)?
            .filter_map(std::result::Result::ok)
            .map(|v| v.path())
            .find(|v| {
                v.file_name().as_ref().is_some_and(|v| {
                    v.as_bytes()
                        .windows(self.0.len())
                        // NOTE this will likely be a problem if we ever decide to support windows at some point
                        .any(|window| window == self.0.as_bytes())
                })
            }))
    }
}

impl FromStr for VideoId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let url = url::Url::parse(s)?;

        let Some(host) = url.host_str() else {
            bail!("Invalid youtube video url: '{}'. No hostname found.", s,);
        };

        if !host.contains("youtube.com") {
            bail!("Invalid youtube video url: '{}'. Received hostname: '{}'", s, host);
        }

        let Some(segments) = url.path_segments().map(Itertools::collect_vec) else {
            bail!("Invalid youtube video url: '{}'", s);
        };

        if !segments.contains(&"watch") {
            bail!("Invalid youtube video url: '{}'", s);
        }

        url.query_pairs()
            .find(|(k, _)| k == "v")
            .map(|(_, v)| Self(v.to_string()))
            .ok_or_else(|| anyhow!("No video id found in url"))
    }
}
