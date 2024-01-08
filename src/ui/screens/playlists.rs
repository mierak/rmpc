use anyhow::{anyhow, Context, Result};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::Rect, widgets::ListItem, Frame};
use strum::Display;
use tracing::instrument;

use crate::{
    mpd::{
        client::Client,
        mpd_client::{Filter, MpdClient, Ranges, SingleOrRange, Tag},
    },
    state::State,
    ui::{
        modals::{rename_playlist::RenamePlaylistModal, Modals},
        utils::dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        KeyHandleResultInternal, Level, SharedUiState, StatusMessage,
    },
};

use super::{browser::DirOrSong, CommonAction, Screen, SongExt};

#[derive(Debug, Default)]
pub struct PlaylistsScreen {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum PlaylistsActions {
    Add,
    Delete,
    Rename,
}

impl PlaylistsScreen {
    fn prepare_preview(&mut self, client: &mut Client<'_>, state: &State) -> Result<()> {
        if let Some(current) = self.stack.current().selected() {
            let p = match current {
                DirOrSong::Dir(d) => client
                    .list_playlist(d)?
                    .into_iter()
                    .map(DirOrSong::Song)
                    .map(|s| s.to_list_item(&state.config.symbols, false))
                    .collect::<Vec<ListItem<'static>>>(),
                DirOrSong::Song(file) => client
                    .find_one(&[Filter::new(Tag::File, file)])?
                    .context(anyhow!("File '{file}' was listed but not found"))?
                    .to_preview(&state.config.symbols)
                    .collect(),
            };
            self.stack.set_preview(Some(p));
        }
        Ok(())
    }

    fn next(&mut self, client: &mut Client<'_>, shared: &mut SharedUiState) -> Result<()> {
        let Some(selected) = self.stack.current().selected() else {
            tracing::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };

        match selected {
            DirOrSong::Dir(playlist) => {
                let info = client.list_playlist(playlist)?;
                self.stack.push(info.into_iter().map(DirOrSong::Song).collect());
            }
            DirOrSong::Song(song) => {
                client.add(song)?;
                if let Ok(Some(song)) = client.find_one(&[Filter::new(Tag::File, song)]) {
                    shared.status_message = Some(StatusMessage::new(
                        format!("'{}' by '{}' added to queue", song.title_str(), song.artist_str()),
                        Level::Info,
                    ));
                }
            }
        }

        return Ok(());
    }
}

impl Screen for PlaylistsScreen {
    type Actions = PlaylistsActions;
    fn render<B: ratatui::prelude::Backend>(
        &mut self,
        frame: &mut Frame<B>,
        area: Rect,
        app: &mut State,
        _shared_state: &mut SharedUiState,
    ) -> Result<()> {
        frame.render_stateful_widget(
            Browser::new(&app.config.symbols).set_widths(&app.config.column_widths),
            area,
            &mut self.stack,
        );

        Ok(())
    }

    #[instrument(err)]
    fn before_show(
        &mut self,
        client: &mut Client<'_>,
        app: &mut crate::state::State,
        shared: &mut SharedUiState,
    ) -> Result<()> {
        let mut playlists: Vec<_> = client
            .list_playlists()
            .context("Cannot list playlists")?
            .into_iter()
            .map(|playlist| DirOrSong::Dir(playlist.name))
            .collect();
        playlists.sort();
        self.stack = DirStack::new(playlists);
        self.prepare_preview(client, app).context("Cannot prepare preview")?;
        Ok(())
    }

    #[instrument(err)]
    fn refresh(
        &mut self,
        client: &mut Client<'_>,
        app: &mut crate::state::State,
        shared: &mut SharedUiState,
    ) -> Result<()> {
        let selected_idx = self.stack.current().selected_with_idx().map(|(_, idx)| idx);
        let filter = std::mem::take(&mut self.stack.current_mut().filter);
        match self.stack.pop() {
            Some(_) => {
                self.next(client, shared)?;
            }
            None => {
                self.before_show(client, app, shared)?;
            }
        };
        self.stack.current_mut().state.select(selected_idx);
        self.stack.current_mut().filter = filter;
        self.prepare_preview(client, app)
            .context("Cannot prepare preview after refresh")?;

        Ok(())
    }

    #[instrument(err)]
    fn handle_action(
        &mut self,
        event: KeyEvent,
        client: &mut Client<'_>,
        app: &mut State,
        shared: &mut SharedUiState,
    ) -> Result<KeyHandleResultInternal> {
        if self.filter_input_mode {
            match event.code {
                KeyCode::Char(c) => {
                    if let Some(ref mut f) = self.stack.current_mut().filter {
                        f.push(c);
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Backspace => {
                    if let Some(ref mut f) = self.stack.current_mut().filter {
                        f.pop();
                    };
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Enter => {
                    self.filter_input_mode = false;
                    self.stack.current_mut().jump_next_matching();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Esc => {
                    self.filter_input_mode = false;
                    self.stack.current_mut().filter = None;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                _ => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else if let Some(action) = app.config.keybinds.playlists.get(&event.into()) {
            match action {
                PlaylistsActions::Add => {
                    if let Some(playlist) = self.stack.current().selected() {
                        match playlist {
                            DirOrSong::Dir(d) => {
                                client.load_playlist(d)?;
                                shared.status_message = Some(StatusMessage::new(
                                    format!("Playlist '{d}' added to queue"),
                                    Level::Info,
                                ));
                            }
                            DirOrSong::Song(s) => {
                                client.add(s)?;
                                if let Ok(Some(song)) = client.find_one(&[Filter::new(Tag::File, s)]) {
                                    shared.status_message = Some(StatusMessage::new(
                                        format!("'{}' by '{}' added to queue", song.title_str(), song.artist_str()),
                                        Level::Info,
                                    ));
                                }
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
                PlaylistsActions::Delete if !self.stack.current().marked().is_empty() => {
                    for idx in self.stack.current().marked() {
                        let item = &self.stack.current().items[*idx];
                        match item {
                            DirOrSong::Dir(d) => {
                                client.delete_playlist(d)?;
                                shared.status_message =
                                    Some(StatusMessage::new(format!("Playlist '{d}' deleted"), Level::Info));
                            }
                            DirOrSong::Song(s) => {
                                let Some(DirOrSong::Dir(playlist)) = self.stack.previous().selected() else {
                                    return Ok(KeyHandleResultInternal::SkipRender);
                                };
                                client.delete_from_playlist(playlist, &SingleOrRange::single(*idx))?;
                                shared.status_message = Some(StatusMessage::new(
                                    format!("File '{s}' deleted from playlist '{playlist}'"),
                                    Level::Info,
                                ));
                            }
                        }
                    }
                    self.refresh(client, app, shared)?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                PlaylistsActions::Delete => match self.stack.current().selected_with_idx() {
                    Some((DirOrSong::Dir(d), _)) => {
                        client.delete_playlist(d)?;
                        shared.status_message =
                            Some(StatusMessage::new(format!("Playlist '{d}' deleted"), Level::Info));
                        self.refresh(client, app, shared)?;
                        Ok(KeyHandleResultInternal::RenderRequested)
                    }
                    Some((DirOrSong::Song(s), idx)) => {
                        let Some(DirOrSong::Dir(playlist)) = self.stack.previous().selected() else {
                            return Ok(KeyHandleResultInternal::SkipRender);
                        };
                        if self.stack.current().marked().is_empty() {
                            client.delete_from_playlist(playlist, &SingleOrRange::single(idx))?;
                            shared.status_message = Some(StatusMessage::new(
                                format!("File '{s}' deleted from playlist '{playlist}'"),
                                Level::Info,
                            ));
                        } else {
                            let ranges: Ranges = self.stack.current().marked().into();
                            for range in ranges.iter().rev() {
                                client.delete_from_playlist(playlist, range)?;
                                shared.status_message = Some(StatusMessage::new(
                                    format!("Songs in ranges '{ranges}' deleted from playlist '{playlist}'",),
                                    Level::Info,
                                ));
                            }
                        }
                        self.refresh(client, app, shared)?;
                        Ok(KeyHandleResultInternal::SkipRender)
                    }
                    None => Ok(KeyHandleResultInternal::SkipRender),
                },
                PlaylistsActions::Rename => match self.stack.current().selected() {
                    Some(DirOrSong::Dir(d)) => Ok(KeyHandleResultInternal::Modal(Some(Modals::RenamePlaylist(
                        RenamePlaylistModal::new(d.clone()),
                    )))),
                    Some(_) => Ok(KeyHandleResultInternal::SkipRender),
                    None => Ok(KeyHandleResultInternal::SkipRender),
                },
            }
        } else if let Some(action) = app.config.keybinds.navigation.get(&event.into()) {
            match action {
                CommonAction::DownHalf => {
                    self.stack.current_mut().next_half_viewport();
                    self.prepare_preview(client, app).context("Cannot prepare preview")?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::UpHalf => {
                    self.stack.current_mut().prev_half_viewport();
                    self.prepare_preview(client, app).context("Cannot prepare preview")?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Up => {
                    self.stack.current_mut().prev();
                    self.prepare_preview(client, app).context("Cannot prepare preview")?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    self.stack.current_mut().next();
                    self.prepare_preview(client, app).context("Cannot prepare preview")?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Bottom => {
                    self.stack.current_mut().last();
                    self.prepare_preview(client, app)?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Top => {
                    self.stack.current_mut().first();
                    self.prepare_preview(client, app)?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Right => {
                    self.next(client, shared)?;
                    self.prepare_preview(client, app).context("Cannot prepare preview")?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Left => {
                    self.stack.pop();
                    self.prepare_preview(client, app).context("Cannot prepare preview")?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::EnterSearch => {
                    self.filter_input_mode = true;
                    self.stack.current_mut().filter = Some(String::new());
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::NextResult => {
                    self.stack.current_mut().jump_next_matching();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::PreviousResult => {
                    self.stack.current_mut().jump_previous_matching();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Select => {
                    self.stack.current_mut().toggle_mark_selected();
                    self.stack.current_mut().next();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
            }
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}
