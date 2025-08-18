use derive_more::AsRef;
use serde::Serialize;

use crate::mpd::{FromMpd, LineHandled, errors::MpdError};

#[derive(Debug, Serialize, Default, PartialEq, AsRef, Clone, Copy)]
pub struct Volume(u32);

impl Bound<u32> for Volume {
    fn value(&self) -> &u32 {
        &self.0
    }

    fn set_value(&mut self, value: u32) -> &Self {
        self.0 = value.clamp(0, 100);
        self
    }

    fn inc(&mut self) -> &Self {
        if self.0 < 100 {
            self.0 += 1;
        }
        self
    }

    fn inc_by(&mut self, step: u32) -> &Self {
        self.0 = self.0.saturating_add(step).min(100);
        self
    }

    fn dec(&mut self) -> &Self {
        if self.0 > 0 {
            self.0 -= 1;
        }
        self
    }

    fn dec_by(&mut self, step: u32) -> &Self {
        self.0 = self.0.saturating_sub(step);
        self
    }
}

impl Volume {
    pub fn new(value: u32) -> Self {
        Self(value.clamp(0, 100))
    }
}

#[allow(dead_code)]
pub trait Bound<T> {
    fn value(&self) -> &T;
    fn set_value(&mut self, value: T) -> &Self;
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
}
