use std::str::FromStr;

use anyhow::{Context, Error, anyhow};
use derive_more::Into;

#[derive(Debug, derive_more::Deref, Into, Clone, Copy, Eq, PartialEq)]
#[into(u16, u32, u64, u128)]
pub struct Percent(u16);

impl FromStr for Percent {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(
            s.strip_suffix("%")
                .context(anyhow!("Invalid percent format '{}'", s))?
                .parse()
                .context(anyhow!("Invalid percent format '{}'", s))?,
        ))
    }
}
