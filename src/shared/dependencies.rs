use std::{process::Command, sync::LazyLock};

use crate::config::MpdAddress;

pub static FFMPEG: LazyLock<Dep> = LazyLock::new(|| Dep::new("ffmpeg", "ffmpeg", &["-version"]));
pub static FFPROBE: LazyLock<Dep> = LazyLock::new(|| Dep::new("ffprobe", "ffprobe", &["-version"]));
pub static YTDLP: LazyLock<Dep> = LazyLock::new(|| Dep::new("yt-dlp", "yt-dlp", &["--version"]));
pub static UEBERZUGPP: LazyLock<Dep> = LazyLock::new(|| Dep::new("ueberzugpp", "ueberzugpp", &["--version"]));
pub static PYTHON3: LazyLock<Dep> = LazyLock::new(|| Dep::new("python3", "python3", &["--version"]));
pub static PYTHON3MUTAGEN: LazyLock<Dep> = LazyLock::new(|| {
    Dep::new(
        "python-mutagen",
        "python3",
        &[
            "-c",
            "try:\n\timport mutagen\n\tprint(\"PRESENT\")\nexcept ImportError:\n\tprint(\"NOT PRESENT\")",
        ],
    )
});

pub static DEPENDENCIES: [&std::sync::LazyLock<Dep>; 6] =
    [&FFMPEG, &FFPROBE, &YTDLP, &UEBERZUGPP, &PYTHON3, &PYTHON3MUTAGEN];

pub fn is_youtube_supported(mpd_address: MpdAddress) -> Result<(), Vec<String>> {
    let mut unsupported = Vec::new();
    if !YTDLP.installed {
        unsupported.push("yt-dlp".to_string());
    }
    if !FFMPEG.installed {
        unsupported.push("ffmpeg".to_string());
    }
    if !FFPROBE.installed {
        unsupported.push("ffprobe".to_string());
    }
    if !PYTHON3.installed {
        unsupported.push("python3".to_string());
    }
    if !PYTHON3MUTAGEN.installed {
        unsupported.push("python-mutagen".to_string());
    }
    if matches!(mpd_address, MpdAddress::IpAndPort(_)) {
        unsupported.push("socket connection to MPD".to_string());
    }

    if unsupported.is_empty() {
        Ok(())
    } else {
        Err(unsupported)
    }
}

pub struct Dep {
    pub name: &'static str,
    pub installed: bool,
    pub version: String,
}

impl Dep {
    fn new(name: &'static str, bin: &'static str, version_args: &'static [&str]) -> Self {
        let mut installed = which::which(bin).is_ok();
        let version = if installed {
            Command::new(bin)
                .args(version_args)
                .output()
                .ok()
                .map(|output| {
                    String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .to_string()
                })
                .unwrap_or_default()
        } else {
            "Version not available".to_string()
        };
        if version == "NOT PRESENT" {
            installed = false;
        }

        Self {
            name,
            installed,
            version,
        }
    }

    pub fn display(&self) -> String {
        format!(
            "{:<20} {:<15} {:<20}",
            self.name,
            if self.installed { "installed" } else { "not installed" },
            self.version
        )
    }

    pub fn log(&self) {
        log::info!(name = self.name, installed = self.installed, version = self.version.as_str(); "Dependency check");
    }
}
