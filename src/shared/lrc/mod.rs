mod index;
mod lyrics;

use std::time::Duration;

use anyhow::Context;
pub use index::{LrcIndex, LrcIndexEntry};
pub use lyrics::Lrc;

fn parse_length(input: &str) -> anyhow::Result<Duration> {
    let (minutes, seconds) = input.split_once(':').context("Invalid lrc length format")?;
    let minutes: u64 = minutes.parse().context("Invalid minutes format in lrc length")?;
    let seconds: u64 = seconds.parse().context("Invalid seconds format in lrc length")?;
    Ok(Duration::from_secs(minutes * 60 + seconds))
}
