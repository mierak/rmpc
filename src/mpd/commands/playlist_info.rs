use super::Song;
use anyhow::{Context, Result};

pub const COMMAND: &[u8; 12] = b"playlistinfo";

#[derive(Debug, Default)]
pub struct PlayListInfo(pub Vec<Song>);

impl std::str::FromStr for PlayListInfo {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut acc = vec![];

        for line in s.lines() {
            if line.starts_with("file:") {
                acc.push(String::new());
            }
            acc.last_mut()
                .context("No element in accumulator while parsing PlayListInfo")?
                .push_str(line);
            acc.last_mut()
                .context("No element in accumulator while parsing PlayListInfo")?
                .push('\n');
        }

        let res = acc.iter().map(|s| Song::from_str(s)).collect::<Result<Vec<Song>>>()?;
        Ok(Self(res))
    }
}
