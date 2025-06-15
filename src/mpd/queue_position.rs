#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum QueuePosition {
    /// relative to the currently playing song; e.g. +0 moves to right after the
    /// current song
    RelativeAdd(usize),
    /// relative to the currently playing song; e.g. -0 moves to right before
    /// the current song
    RelativeSub(usize),
    Absolute(usize),
}

impl QueuePosition {
    #[must_use]
    pub fn as_mpd_str(&self) -> String {
        match self {
            QueuePosition::RelativeAdd(v) => format!("+{v}"),
            QueuePosition::RelativeSub(v) => format!("-{v}"),
            QueuePosition::Absolute(v) => format!("{v}"),
        }
    }
}

impl std::str::FromStr for QueuePosition {
    type Err = anyhow::Error;

    fn from_str(v: &str) -> anyhow::Result<Self> {
        let f =
            v.chars().nth(0).ok_or(anyhow::anyhow!("Invalid (empty) position string: '{v}'"))?;
        Ok(match f {
            '+' => QueuePosition::RelativeAdd(parse_subsequent(v)?),
            '-' => QueuePosition::RelativeSub(parse_subsequent(v)?),
            _ => Self::Absolute(v.parse()?),
        })
    }
}

fn parse_subsequent(v: &str) -> anyhow::Result<usize> {
    Ok(v.get(1..)
        .ok_or(anyhow::anyhow!("Invalid position string: '{v}'. Please add a number."))?
        .parse()?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn test_queue_position_fromstr() {
        assert!("+0".parse::<QueuePosition>().unwrap() == QueuePosition::RelativeAdd(0));
        assert!("-0".parse::<QueuePosition>().unwrap() == QueuePosition::RelativeSub(0));
        assert!("-15".parse::<QueuePosition>().unwrap() == QueuePosition::RelativeSub(15));
        assert!("0".parse::<QueuePosition>().unwrap() == QueuePosition::Absolute(0));
        assert!("15".parse::<QueuePosition>().unwrap() == QueuePosition::Absolute(15));
    }
}
