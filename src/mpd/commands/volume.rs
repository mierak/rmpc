use crate::mpd::errors::MpdError;
use crate::mpd::{FromMpd, LineHandled};

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
