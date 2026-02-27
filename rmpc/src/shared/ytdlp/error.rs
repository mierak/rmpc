use thiserror::Error;

use crate::shared::ytdlp::ytdlp_item::YtDlpHost;

#[derive(Error, Debug)]
pub enum YtDlpDownloadError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("yt-dlp exited with code: {code:?}: stdout: {stdout}, stderr: {stderr}")]
    YtDlpError { stdout: String, stderr: String, code: Option<i32> },
    #[error(
        "Did not find file downloaded by yt-dlp in cache directory, yt-dlp exited with code: {code:?}: stdout: {stdout}, stderr: {stderr}"
    )]
    FileNotFound { stdout: String, stderr: String, code: Option<i32> },
    #[error("Invalid yt-dlp configuration: {0}")]
    InvalidConfig(&'static str),
}

#[derive(Error, Debug)]
pub enum YtDlpParseError {
    #[error("Invalid URL: {0}")]
    ParseError(#[from] url::ParseError),
    #[error("Invalid URL, no hostname in '{url}'")]
    NoHost { url: String },
    #[error("Invalid {host} URL: '{url}', {msg}")]
    InvalidFormat { host: YtDlpHost, url: String, msg: &'static str },
    #[error("Unsupported host {host} in URL '{url}'")]
    UnsupportedHost { url: String, host: String },
}

impl YtDlpParseError {
    pub fn invalid_yt(url: &str, msg: &'static str) -> Self {
        Self::InvalidFormat { host: YtDlpHost::Youtube, url: url.to_string(), msg }
    }

    pub fn invalid_sc(url: &str, msg: &'static str) -> Self {
        Self::InvalidFormat { host: YtDlpHost::Soundcloud, url: url.to_string(), msg }
    }

    pub fn invalid_nv(url: &str, msg: &'static str) -> Self {
        Self::InvalidFormat { host: YtDlpHost::NicoVideo, url: url.to_string(), msg }
    }
}
