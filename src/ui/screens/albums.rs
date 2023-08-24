use crate::{
    mpd::{client::Client, errors::MpdError},
    state::State,
    ui::{Render, SharedUiState},
};

use super::Screen;
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::KeyEvent;
use ratatui::{prelude::Rect, Frame};

#[derive(Default, Debug)]
pub struct AlbumsScreen {}

#[async_trait]
impl Screen for AlbumsScreen {
    fn render<B: ratatui::prelude::Backend>(
        &mut self,
        frame: &mut Frame<B>,
        area: Rect,
        app: &mut State,
        shared_state: &mut SharedUiState,
    ) -> Result<()> {
        Ok(())
    }

    async fn handle_key(
        &mut self,
        key: KeyEvent,
        _client: &mut Client<'_>,
        _app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<Render, MpdError> {
        Ok(Render::Skip)
    }
}
