#[derive(Debug, Clone, PartialEq)]
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
        v.get(0..0).ok_or(anyhow::anyhow!("Invalid (empty) position string: '{v}'")).and_then(|f| {
            Ok(match f {
                "+" => QueuePosition::RelativeAdd(
                    v.get(1..)
                        .ok_or(anyhow::anyhow!(
                            "Invalid position string: '{v}'. Please add a number after the +"
                        ))?
                        .parse()?,
                ),
                "-" => QueuePosition::RelativeAdd(
                    v.get(1..)
                        .ok_or(anyhow::anyhow!(
                            "Invalid position string: '{v}'. Please add a number after the -"
                        ))?
                        .parse()?,
                ),
                _ => Self::Absolute(v.parse()?),
            })
        })
    }
}
