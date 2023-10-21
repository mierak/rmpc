use derive_more::AsRef;

use crate::mpd::errors::MpdError;
use crate::mpd::{FromMpd, LineHandled};

#[derive(Debug, Default, PartialEq, AsRef)]
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
    fn inc_by(&mut self, step: u8) -> &Self {
        self.0 = self.0.saturating_add(step).min(100);
        self
    }
    fn dec(&mut self) -> &Self {
        if self.0 > 0 {
            self.0 -= 1;
        }
        self
    }
    fn dec_by(&mut self, step: u8) -> &Self {
        self.0 = self.0.saturating_sub(step).max(0);
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
    fn inc_by(&mut self, step: T) -> &Self;
    fn dec(&mut self) -> &Self;
    fn dec_by(&mut self, step: T) -> &Self;
}

impl FromMpd for Volume {
    fn next_internal(&mut self, key: &str, value: String) -> Result<LineHandled, MpdError> {
        if key == "volume" {
            self.0 = value.parse()?;
            Ok(LineHandled::Yes)
        } else {
            Ok(LineHandled::No { value })
        }
    }

    fn finish(self) -> Result<Self, MpdError> {
        Ok(self)
    }
}
