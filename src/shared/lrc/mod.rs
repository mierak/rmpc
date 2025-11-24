mod index;
mod lyrics;

use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result, bail};
pub use index::LrcIndex;
pub use lyrics::{Lrc, LrcMetadata};

#[derive(Debug, Default, Clone, Copy)]
pub struct LrcOffset {
    negative: bool,
    value: Duration,
}

impl LrcOffset {
    pub fn from_millis(value: i64) -> Self {
        if value < 0 {
            Self { negative: true, value: Duration::from_millis(-value as u64) }
        } else {
            Self { negative: false, value: Duration::from_millis(value as u64) }
        }
    }
}

fn parse_length(input: &str) -> anyhow::Result<Duration> {
    let (minutes, seconds) = input.split_once(':').context("Invalid lrc length format")?;
    let minutes: u64 = minutes.parse().context("Invalid minutes format in lrc length")?;
    let seconds: u64 = seconds.parse().context("Invalid seconds format in lrc length")?;
    Ok(Duration::from_secs(minutes * 60 + seconds))
}

pub(crate) fn get_lrc_path(lyrics_dir: &str, song_file: &str) -> Result<PathBuf> {
    let mut path: PathBuf = PathBuf::from(lyrics_dir);
    path.push(song_file);
    let Some(stem) = path.file_stem().map(|stem| format!("{}.lrc", stem.to_string_lossy())) else {
        bail!("No file stem for lyrics path: {path:?}");
    };

    path.pop();
    path.push(stem);
    Ok(path)
}
