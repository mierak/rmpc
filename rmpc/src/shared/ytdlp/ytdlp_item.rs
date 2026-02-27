use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::Result;
use itertools::Itertools;
use rustix::path::Arg;
use walkdir::WalkDir;

use crate::shared::ytdlp::error::YtDlpParseError;

#[derive(Debug, Clone)]
pub struct YtDlpItem {
    /// id of the video/audio, to be used in the url
    pub id: String,
    /// filename of the video/audio, will be used to cache the file
    pub filename: String,
    pub kind: YtDlpHost,
}

pub struct YtDlpPlaylist {
    pub kind: YtDlpHost,
    pub id: String,
}

pub enum YtDlpContent {
    Single(YtDlpItem),
    Playlist(YtDlpPlaylist),
}

#[derive(Clone, Copy, Debug, strum::AsRefStr, strum::Display)]
pub enum YtDlpHost {
    Youtube,
    Soundcloud,
    NicoVideo,
}

impl YtDlpItem {
    pub fn to_url(&self) -> String {
        self.kind.watch_url(&self.id)
    }

    pub fn cache_subdir(&self, root: &Path) -> PathBuf {
        root.join(self.kind.cache_dir_name())
    }

    pub fn get_cached(&self, cache_dir: &Path) -> Option<PathBuf> {
        WalkDir::new(self.cache_subdir(cache_dir))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .find(|e| e.path().file_stem().is_some_and(|stem| stem == OsStr::new(&self.filename)))
            .map(|entry| entry.into_path())
    }

    pub fn delete_cached(&self, cache_dir: &Path) -> Result<()> {
        let files = WalkDir::new(self.cache_subdir(cache_dir))
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

impl YtDlpHost {
    fn cache_dir_name(self) -> &'static str {
        match self {
            Self::Youtube => "youtube",
            Self::Soundcloud => "soundcloud",
            Self::NicoVideo => "nicovideo",
        }
    }

    pub fn watch_url(self, id: &str) -> String {
        match self {
            Self::Youtube => format!("https://www.youtube.com/watch?v={id}"),
            Self::NicoVideo => format!("https://www.nicovideo.jp/watch/{id}"),
            Self::Soundcloud => {
                if id.contains('/') {
                    format!("https://soundcloud.com/{id}")
                } else if id.chars().all(|c| c.is_ascii_digit()) {
                    format!("https://api.soundcloud.com/tracks/{id}")
                } else {
                    // fallback to web
                    format!("https://soundcloud.com/{id}")
                }
            }
        }
    }

    pub fn search_key(self) -> &'static str {
        match self {
            Self::Youtube => "ytsearch",
            Self::Soundcloud => "scsearch",
            Self::NicoVideo => "nicosearch",
        }
    }
}

impl FromStr for YtDlpContent {
    type Err = YtDlpParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let url = url::Url::parse(s)?;

        let Some(host) = url.host_str() else {
            return Err(YtDlpParseError::NoHost { url: s.to_string() });
        };

        match host.strip_prefix("www.").unwrap_or(host) {
            "youtube.com" | "music.youtube.com" => {
                let segments = url
                    .path_segments()
                    .ok_or_else(|| YtDlpParseError::invalid_yt(s, "cannot-be-a-base"))?
                    .collect_vec();

                let is_watch_url = segments.contains(&"watch");
                let is_playlist_url = segments.contains(&"playlist")
                    || url.query_pairs().any(|(key, _)| key == "list");

                if is_playlist_url {
                    url.query_pairs()
                        .find(|(k, _)| k == "list")
                        .map(|(_, v)| YtDlpPlaylist { id: v.to_string(), kind: YtDlpHost::Youtube })
                        .ok_or_else(|| YtDlpParseError::invalid_yt(s, "no playlist id found"))
                        .map(YtDlpContent::Playlist)
                } else if is_watch_url {
                    url.query_pairs()
                        .find(|(k, _)| k == "v")
                        .map(|(_, v)| YtDlpItem {
                            id: v.to_string(),
                            filename: v.to_string(),
                            kind: YtDlpHost::Youtube,
                        })
                        .ok_or_else(|| YtDlpParseError::invalid_yt(s, "no video id found"))
                        .map(YtDlpContent::Single)
                } else {
                    return Err(YtDlpParseError::invalid_yt(s, "unrecognized youtube url format"));
                }
            }
            "youtu.be" => url
                .path_segments()
                .ok_or_else(|| YtDlpParseError::invalid_yt(s, "cannot-be-a-base"))?
                .next()
                .map(|x| YtDlpItem {
                    id: x.to_string(),
                    filename: x.to_string(),
                    kind: YtDlpHost::Youtube,
                })
                .ok_or_else(|| YtDlpParseError::invalid_yt(s, "no video id found"))
                .map(YtDlpContent::Single),
            "soundcloud.com" | "api.soundcloud.com" => {
                let mut path_segments = url
                    .path_segments()
                    .ok_or_else(|| YtDlpParseError::invalid_sc(s, "cannot-be-a-base"))?;
                let Some(first) = path_segments.next() else {
                    return Err(YtDlpParseError::invalid_sc(s, "no path segments found"));
                };

                if first == "tracks" {
                    // API form: https://api.soundcloud.com/tracks/<id>
                    let Some(track_id) = path_segments.next() else {
                        return Err(YtDlpParseError::invalid_sc(s, "no track id found"));
                    };

                    Ok(YtDlpContent::Single(YtDlpItem {
                        id: track_id.to_string(),
                        filename: track_id.to_string(),
                        kind: YtDlpHost::Soundcloud,
                    }))
                } else {
                    // Web form: https://soundcloud.com/<user>/<track>
                    let username = first;
                    let Some(track_name) = path_segments.next() else {
                        return Err(YtDlpParseError::invalid_sc(s, "no track name found"));
                    };

                    Ok(YtDlpContent::Single(YtDlpItem {
                        id: format!("{username}/{track_name}"),
                        filename: format!("{username}-{track_name}"),
                        kind: YtDlpHost::Soundcloud,
                    }))
                }
            }
            "nicovideo.jp" => {
                let mut path_segments = url
                    .path_segments()
                    .ok_or_else(|| YtDlpParseError::invalid_nv(s, "cannot-be-a-base"))?;

                let Some(_watch_segment) = path_segments.next() else {
                    return Err(YtDlpParseError::invalid_nv(s, "no watch segment"));
                };

                let Some(id) = path_segments.next() else {
                    return Err(YtDlpParseError::invalid_nv(s, "no video id found"));
                };

                Ok(YtDlpContent::Single(YtDlpItem {
                    id: id.to_string(),
                    filename: id.to_string(),
                    kind: YtDlpHost::NicoVideo,
                }))
            }
            _ => {
                Err(YtDlpParseError::UnsupportedHost { host: host.to_string(), url: s.to_string() })
            }
        }
    }
}
