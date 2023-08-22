use std::io::Stdout;

use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::KeyEvent;
use ratatui::{
    prelude::{CrosstermBackend, Rect},
    Frame,
};
use strum::{Display, EnumIter, EnumVariantNames};

use crate::{
    mpd::{client::Client, errors::MpdError},
    state::State,
};

use super::{Render, SharedUiState};

pub mod directories;
pub mod logs;
pub mod queue;

#[derive(Debug, Display, EnumVariantNames, Default, Clone, Copy, EnumIter, PartialEq)]
pub enum Screens {
    #[default]
    Queue,
    Logs,
    Directories,
}

#[async_trait]
pub trait Screen {
    fn render(
        &mut self,
        frame: &mut Frame<CrosstermBackend<Stdout>>,
        area: Rect,
        app: &mut crate::state::State,
        shared_state: &mut SharedUiState,
    ) -> Result<()>;

    /// For any cleanup operations, ran when the screen hides
    async fn on_hide(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared_state: &mut SharedUiState,
    ) -> Result<()> {
        Ok(())
    }

    /// For work that needs to be done BEFORE the first render
    async fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        Ok(())
    }

    async fn handle_key(
        &mut self,
        key: KeyEvent,
        _client: &mut Client<'_>,
        _app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<Render, MpdError>;
}

impl Screens {
    pub fn next(&self) -> Self {
        match self {
            Screens::Queue => Screens::Logs,
            Screens::Logs => Screens::Directories,
            Screens::Directories => Screens::Queue,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Screens::Queue => Screens::Directories,
            Screens::Directories => Screens::Logs,
            Screens::Logs => Screens::Queue,
        }
    }
}
