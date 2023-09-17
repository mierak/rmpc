use anyhow::{Context, Result};
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::Rect, widgets::ListItem, Frame};
use tracing::instrument;

use crate::{
    mpd::{client::Client, mpd_client::MpdClient},
    state::State,
    ui::{widgets::browser::Browser, KeyHandleResult, Level, SharedUiState, StatusMessage},
};

use super::{browser::DirOrSongInfo, browser::ToListItems, dirstack::DirStack, CommonAction, Screen, SongExt};

#[derive(Debug)]
pub struct PlaylistsScreen {
    stack: DirStack<DirOrSongInfo>,
    filter_input_mode: bool,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum PlaylistsActions {}

impl Default for PlaylistsScreen {
    fn default() -> Self {
        Self {
            stack: DirStack::new(Vec::new()),
            filter_input_mode: false,
        }
    }
}

impl PlaylistsScreen {
    async fn prepare_preview(&mut self, client: &mut Client<'_>, state: &State) -> Result<Vec<ListItem<'static>>> {
        let idx = self
            .stack
            .current()
            .1
            .get_selected()
            .context("Expected an item to be selected")?;
        let current = &self.stack.current().0[idx];
        match current {
            DirOrSongInfo::Dir(d) => {
                let res = client
                    .list_playlist_info(d)
                    .await?
                    .into_iter()
                    .map(|s| ListItem::new(s.title_str().to_owned()))
                    .collect();
                Ok(res)
            }
            DirOrSongInfo::Song(s) => Ok(s.to_listitems(&state.config.symbols)),
        }
    }
}

#[async_trait]
impl Screen for PlaylistsScreen {
    type Actions = PlaylistsActions;
    fn render<B: ratatui::prelude::Backend>(
        &mut self,
        frame: &mut Frame<B>,
        area: Rect,
        app: &mut State,
        _shared_state: &mut SharedUiState,
    ) -> Result<()> {
        let w = Browser::new(&app.config.symbols, &app.config.column_widths);
        frame.render_stateful_widget(w, area, &mut self.stack);

        Ok(())
    }

    #[instrument(err)]
    async fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        let playlists = _client.list_playlists().await.context("Cannot list playlists")?;
        self.stack = DirStack::new(
            playlists
                .into_iter()
                .map(|playlist| DirOrSongInfo::Dir(playlist.name))
                .collect(),
        );
        self.stack.preview = self
            .prepare_preview(_client, _app)
            .await
            .context("Cannot prepare preview")?;
        Ok(())
    }

    #[instrument(err)]
    async fn handle_action(
        &mut self,
        event: KeyEvent,
        client: &mut Client<'_>,
        app: &mut State,
        shared: &mut SharedUiState,
    ) -> Result<KeyHandleResult> {
        if self.filter_input_mode {
            match event.code {
                KeyCode::Char(c) => {
                    if let Some(ref mut f) = self.stack.filter {
                        f.push(c);
                    }
                    Ok(KeyHandleResult::RenderRequested)
                }
                KeyCode::Backspace => {
                    if let Some(ref mut f) = self.stack.filter {
                        f.pop();
                    };
                    Ok(KeyHandleResult::RenderRequested)
                }
                KeyCode::Enter => {
                    self.filter_input_mode = false;
                    self.stack.jump_forward();
                    Ok(KeyHandleResult::RenderRequested)
                }
                KeyCode::Esc => {
                    self.filter_input_mode = false;
                    self.stack.filter = None;
                    Ok(KeyHandleResult::RenderRequested)
                }
                _ => Ok(KeyHandleResult::SkipRender),
            }
        } else if let Some(action) = app.config.keybinds.artists.get(&event.into()) {
            match action {
                _ => Ok(KeyHandleResult::SkipRender),
            }
        } else if let Some(action) = app.config.keybinds.navigation.get(&event.into()) {
            match action {
                CommonAction::DownHalf => {
                    self.stack.next_half_viewport();
                    self.stack.preview = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::UpHalf => {
                    self.stack.prev_half_viewport();
                    self.stack.preview = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Up => {
                    self.stack.prev();
                    self.stack.preview = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Down => {
                    self.stack.next();
                    self.stack.preview = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Bottom => {
                    self.stack.last();
                    self.prepare_preview(client, app).await?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Top => {
                    self.stack.first();
                    self.prepare_preview(client, app).await?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Right => {
                    let idx = self
                        .stack
                        .current()
                        .1
                        .get_selected()
                        .context("Expected an item to be selected")?;

                    match &self.stack.current().0[idx] {
                        DirOrSongInfo::Dir(playlist) => {
                            let info = client.list_playlist_info(playlist).await?;
                            self.stack.push(info.into_iter().map(DirOrSongInfo::Song).collect());
                        }
                        DirOrSongInfo::Song(song) => {
                            client.add(&song.file).await?;
                            shared.status_message = Some(StatusMessage::new(
                                format!("'{}' by '{}' added to queue", song.title_str(), song.artist_str()),
                                Level::Info,
                            ));
                        }
                    }

                    self.stack.preview = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Left => {
                    self.stack.pop();
                    self.stack.preview = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::EnterSearch => {
                    self.filter_input_mode = true;
                    self.stack.filter = Some(String::new());
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::NextResult => {
                    self.stack.jump_forward();
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::PreviousResult => {
                    self.stack.jump_back();
                    Ok(KeyHandleResult::RenderRequested)
                }
            }
        } else {
            Ok(KeyHandleResult::KeyNotHandled)
        }
    }
}
