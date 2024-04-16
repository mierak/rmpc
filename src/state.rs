use std::collections::VecDeque;

use crate::{
    config::Config,
    mpd::{
        client::Client,
        commands::{Song, Status},
        mpd_client::MpdClient,
    },
    ui::screens::Screens,
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
    pub active_tab: Screens,
    pub status: Status,
    pub current_song: Option<Song>,
    pub logs: MyVecDeque<Vec<u8>>,
    pub status_loop_active: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            config: Box::leak(Box::default()),
            active_tab: Screens::default(),
            status: Status::default(),
            current_song: None,
            logs: MyVecDeque(VecDeque::new()),
            status_loop_active: false,
        }
    }
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "State {{ active_tab: {}, logs_count: {}}}",
            self.active_tab,
            self.logs.len(),
        )
    }
}

impl State {
    pub fn try_new(client: &mut Client<'_>, config: &'static Config) -> Result<Self> {
        let current_song = client.get_current_song()?;
        let status = client.get_status()?;

        Ok(Self {
            config,
            active_tab: Screens::default(),
            status,
            current_song,
            logs: MyVecDeque(VecDeque::new()),
            status_loop_active: false,
        })
    }
}
