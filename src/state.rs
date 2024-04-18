use std::collections::VecDeque;

use crate::{
    config::Config,
    mpd::{client::Client, commands::Status, mpd_client::MpdClient},
};
use anyhow::Result;
use derive_more::{Deref, DerefMut};

#[derive(Deref, DerefMut, Default)]
pub struct MyVecDeque<T>(VecDeque<T>);
impl<T> std::fmt::Debug for MyVecDeque<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MyVecDeque {{ len={} }}", self.len())
    }
}

pub struct State {
    pub config: &'static Config,
    pub status_loop_active: bool,
    pub status: Status,
}

impl Default for State {
    fn default() -> Self {
        Self {
            status: Status::default(),
            config: Box::leak(Box::default()),
            status_loop_active: false,
        }
    }
}

impl State {
    pub fn try_new(client: &mut Client<'_>, config: &'static Config) -> Result<Self> {
        let status = client.get_status()?;

        Ok(Self {
            status,
            config,
            status_loop_active: false,
        })
    }
}
