use anyhow::{Context, Result};
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::Rect, widgets::ListItem, Frame};
use tracing::instrument;

use crate::{
    mpd::{client::Client, mpd_client::MpdClient},
    state::State,
    ui::{
        modals::{rename_playlist::RenamePlaylistModal, Modals},
        widgets::browser::Browser,
        KeyHandleResultInternal, Level, SharedUiState, StatusMessage,
    },
};

use super::{
    browser::{DirOrSongInfo, ToListItems},
    dirstack::DirStack,
    iter::DirOrSongInfoListItems,
    CommonAction, Screen, SongExt,
};

#[derive(Debug)]
pub struct PlaylistsScreen {
    stack: DirStack<DirOrSongInfo>,
    filter_input_mode: bool,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum PlaylistsActions {
    Add,
    DeletePlaylist,
    Rename,
}

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
                    .map(DirOrSongInfo::Song)
                    .listitems(&state.config.symbols)
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
        let prev: Vec<_> = self
            .stack
            .previous()
            .0
            .iter()
            .cloned()
            .listitems(&app.config.symbols)
            .collect();
        let current: Vec<_> = self
            .stack
            .current()
            .0
            .iter()
            .cloned()
            .listitems(&app.config.symbols)
            .collect();
        let preview = &self.stack.preview().clone();
        let w = Browser::new()
            .widths(&app.config.column_widths)
            .previous_items(&prev)
            .current_items(&current)
            .preview(preview);

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
        let mut playlists: Vec<_> = _client
            .list_playlists()
            .await
            .context("Cannot list playlists")?
            .into_iter()
            .map(|playlist| DirOrSongInfo::Dir(playlist.name))
            .collect();
        playlists.sort();
        self.stack = DirStack::new(playlists);
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
    ) -> Result<KeyHandleResultInternal> {
        if self.filter_input_mode {
            match event.code {
                KeyCode::Char(c) => {
                    if let Some(ref mut f) = self.stack.filter {
                        f.push(c);
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Backspace => {
                    if let Some(ref mut f) = self.stack.filter {
                        f.pop();
                    };
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Enter => {
                    self.filter_input_mode = false;
                    self.stack.jump_forward();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Esc => {
                    self.filter_input_mode = false;
                    self.stack.filter = None;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                _ => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else if let Some(action) = app.config.keybinds.playlists.get(&event.into()) {
            match action {
                PlaylistsActions::Add => {
                    if let Some(playlist) = self.stack.get_selected() {
                        match playlist {
                            DirOrSongInfo::Dir(d) => {
                                client.load_playlist(d).await?;
                                shared.status_message = Some(StatusMessage::new(
                                    format!("Playlist '{d}' added to queue"),
                                    Level::Info,
                                ));
                            }
                            DirOrSongInfo::Song(s) => {
                                client.add(&s.file).await?;
                                shared.status_message = Some(StatusMessage::new(
                                    format!("'{}' by '{}' added to queue", s.title_str(), s.artist_str()),
                                    Level::Info,
                                ));
                            }
                        }
                    } else {
                        shared.status_message = Some(StatusMessage::new(
                            "Failed to add playlist/song to current queue because nothing was selected".to_string(),
                            Level::Error,
                        ));
                        tracing::error!(
                            message = "Failed to add playlist/song to current queue because nothing was selected"
                        );
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                PlaylistsActions::DeletePlaylist => match self.stack.get_selected() {
                    Some(DirOrSongInfo::Dir(d)) => {
                        client.delete_playlist(d).await?;
                        shared.status_message =
                            Some(StatusMessage::new(format!("Playlist '{d}' deleted"), Level::Info));
                        // TODO need to refetch playlists
                        Ok(KeyHandleResultInternal::RenderRequested)
                    }
                    Some(_) => Ok(KeyHandleResultInternal::SkipRender),
                    None => Ok(KeyHandleResultInternal::SkipRender),
                },
                PlaylistsActions::Rename => match self.stack.get_selected() {
                    Some(DirOrSongInfo::Dir(d)) => {
                        // shared.visible_modal = Some(Modals::RenamePlaylist(RenamePlaylistModal::new(d.clone())));
                        Ok(KeyHandleResultInternal::Modal(Some(Modals::RenamePlaylist(
                            RenamePlaylistModal::new(d.clone()),
                        ))))
                        // Ok(KeyHandleResult::RenderRequested)
                    }
                    Some(_) => Ok(KeyHandleResultInternal::SkipRender),
                    None => Ok(KeyHandleResultInternal::SkipRender),
                },
            }
        } else if let Some(action) = app.config.keybinds.navigation.get(&event.into()) {
            match action {
                CommonAction::DownHalf => {
                    self.stack.next_half_viewport();
                    self.stack.preview = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::UpHalf => {
                    self.stack.prev_half_viewport();
                    self.stack.preview = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Up => {
                    self.stack.prev();
                    self.stack.preview = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    self.stack.next();
                    self.stack.preview = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Bottom => {
                    self.stack.last();
                    self.prepare_preview(client, app).await?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Top => {
                    self.stack.first();
                    self.prepare_preview(client, app).await?;
                    Ok(KeyHandleResultInternal::RenderRequested)
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
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Left => {
                    self.stack.pop();
                    self.stack.preview = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::EnterSearch => {
                    self.filter_input_mode = true;
                    self.stack.filter = Some(String::new());
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::NextResult => {
                    self.stack.jump_forward();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::PreviousResult => {
                    self.stack.jump_back();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
            }
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}
