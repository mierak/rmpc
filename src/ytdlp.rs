use anyhow::{anyhow, bail, Result};
use itertools::Itertools;
use rustix::path::Arg;
use std::{os::unix::ffi::OsStrExt, path::PathBuf, process::Command, str::FromStr};

#[derive(Debug)]
pub struct YtDlp {
    pub is_available: bool,
    pub cache_dir: String,
}

impl YtDlp {
    pub fn new(cache_dir: &'static str) -> Result<Self> {
        let cache_dir = format!("{cache_dir}youtube/");
        std::fs::create_dir_all(&cache_dir)?;

        Ok(Self {
            is_available: which::which("yt-dlp").is_ok(),
            cache_dir,
        })
    }

    pub fn download(&self, url: &str) -> Result<String> {
        let id: VideoId = url.parse()?;

        if let Some(cached_file) = id.get_cached(&self.cache_dir)? {
            return Ok(cached_file.to_string_lossy().to_string());
        }

        let output_template = format!("{}%(id)s.%(ext)s", self.cache_dir);

        let out = Command::new("yt-dlp")
            .args([
                "-x",
                "--embed-thumbnail",
                "--embed-metadata",
                "-f",
                "bestaudio",
                "--convert-thumbnails",
                "jpg",
                "--output",
                &output_template,
                "--print",
                &output_template,
                "--no-simulate",
                &format!("https://www.youtube.com/watch?v={}", id.0),
            ])
            .output();

        Ok(String::from_utf8_lossy(&out?.stdout).trim().to_string())
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
