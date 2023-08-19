use anyhow::anyhow;
use anyhow::Context;

pub const COMMAND: &[u8; 6] = b"getvol";

#[derive(Debug, Default, PartialEq)]
pub struct Volume(u8);

impl Bound<u8> for Volume {
    fn value(&self) -> &u8 {
        &self.0
    }
    fn inc(&mut self) -> &Self {
        if self.0 < 100 {
            self.0 += 1;
        }
        self
    }
    fn dec(&mut self) -> &Self {
        if self.0 > 0 {
            self.0 -= 1;
        }
        self
    }
}

impl Volume {
    pub fn new(value: u8) -> Self {
        Self(value.max(0).min(100))
    }
}

pub trait Bound<T> {
    fn value(&self) -> &u8;
    fn inc(&mut self) -> &Self;
    fn dec(&mut self) -> &Self;
}

impl std::str::FromStr for Volume {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let res = s
            .trim_end()
            .split_once(' ')
            .context(anyhow!("Invalid value '{}' when parsing Volume - split", s))?;
        let res = res
            .1
            .parse()
            .context(anyhow!("Invalid value '{}' when parsing Volume", s))?;

        Ok(Self::new(res))
    }
}
