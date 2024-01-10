use crate::{
    mpd::{
        client::Client,
        commands::Song,
        errors::MpdError,
        mpd_client::{Filter, MpdClient, Tag},
    },
    state::State,
    ui::{
        utils::dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        KeyHandleResultInternal, Level, SharedUiState, StatusMessage,
    },
};

use super::{browser::DirOrSong, BrowserScreen, Screen};
use anyhow::{Context, Result};
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::{prelude::Rect, widgets::ListItem, Frame};
use strum::Display;
use tracing::instrument;

#[derive(Debug, Default)]
pub struct ArtistsScreen {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
}

impl Screen for ArtistsScreen {
    type Actions = ArtistsActions;
    fn render(
        &mut self,
        frame: &mut Frame,
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
        let result = client.list_tag(Tag::Artist, None).context("Cannot list artists")?;
        self.stack = DirStack::new(result.into_iter().map(DirOrSong::Dir).collect::<Vec<_>>());
        let preview = self.prepare_preview(client, app).context("Cannot prepare preview")?;
        self.stack.set_preview(preview);

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
            self.handle_filter_input(event);
            Ok(KeyHandleResultInternal::RenderRequested)
        } else if let Some(_action) = app.config.keybinds.artists.get(&event.into()) {
            Ok(KeyHandleResultInternal::SkipRender)
        } else if let Some(action) = app.config.keybinds.navigation.get(&event.into()) {
            self.handle_common_action(*action, client, app, shared)
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum ArtistsActions {}

#[tracing::instrument]
fn list_titles(
    client: &mut Client<'_>,
    artist: &str,
    album: &str,
) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .list_tag(
            Tag::Title,
            Some(&[Filter::new(Tag::Artist, artist), Filter::new(Tag::Album, album)]),
        )?
        .into_iter()
        .map(DirOrSong::Song))
}

#[tracing::instrument]
fn list_albums(client: &mut Client<'_>, artist: &str) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .list_tag(Tag::Album, Some(&[Filter::new(Tag::Artist, artist)]))?
        .into_iter()
        .map(DirOrSong::Dir))
}

#[tracing::instrument]
fn find_songs(client: &mut Client<'_>, artist: &str, album: &str, file: &str) -> Result<Vec<Song>, MpdError> {
    client.find(&[
        Filter::new(Tag::Title, file),
        Filter::new(Tag::Artist, artist),
        Filter::new(Tag::Album, album),
    ])
}

#[tracing::instrument]
fn add_song(client: &mut Client<'_>, artist: &str, album: &str, title: &str) -> Result<(), MpdError> {
    client.find_add(&[
        Filter::new(Tag::Title, title),
        Filter::new(Tag::Artist, artist),
        Filter::new(Tag::Album, album),
    ])
}

impl BrowserScreen<DirOrSong> for ArtistsScreen {
    fn stack(&self) -> &DirStack<DirOrSong> {
        &self.stack
    }

    fn stack_mut(&mut self) -> &mut DirStack<DirOrSong> {
        &mut self.stack
    }

    fn set_filter_input_mode_active(&mut self, active: bool) {
        self.filter_input_mode = active;
    }

    fn is_filter_input_mode_active(&self) -> bool {
        self.filter_input_mode
    }

    fn add(
        &self,
        item: &DirOrSong,
        client: &mut Client<'_>,
        shared: &mut SharedUiState,
    ) -> Result<KeyHandleResultInternal> {
        match self.stack.path() {
            [artist, album] => {
                client.find_add(&[
                    Filter::new(Tag::Artist, artist.as_str()),
                    Filter::new(Tag::Album, album.as_str()),
                    Filter::new(Tag::Title, item.value()),
                ])?;

                shared.status_message = Some(StatusMessage::new(
                    format!("'{}' by '{artist}' from album '{album}' added to queue", item.value()),
                    Level::Info,
                ));
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [artist] => {
                client.find_add(&[
                    Filter::new(Tag::Artist, artist.as_str()),
                    Filter::new(Tag::Album, item.value()),
                ])?;

                shared.status_message = Some(StatusMessage::new(
                    format!("Album '{}' by '{artist}' added to queue", item.value()),
                    Level::Info,
                ));
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [] => {
                client.find_add(&[Filter::new(Tag::Artist, item.value())])?;

                shared.status_message = Some(StatusMessage::new(
                    format!("All songs by '{}' added to queue", item.value()),
                    Level::Info,
                ));
                Ok(KeyHandleResultInternal::SkipRender)
            }
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }

    fn next(&mut self, client: &mut Client<'_>, shared: &mut SharedUiState) -> Result<()> {
        let Some(current) = self.stack.current().selected() else {
            tracing::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };
        let Some(value) = current.as_path() else {
            tracing::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };

        match self.stack.path() {
            [artist, album] => {
                add_song(client, artist, album, value)?;

                shared.status_message = Some(StatusMessage::new(
                    format!("'{value}' by '{artist}' added to queue"),
                    Level::Info,
                ));
            }
            [artist] => self.stack.push(list_titles(client, artist, value)?.collect()),
            [] => self.stack.push(list_albums(client, value)?.collect()),
            _ => tracing::error!("Unexpected nesting in Artists dir structure"),
        }
        Ok(())
    }

    fn prepare_preview(&mut self, client: &mut Client<'_>, state: &State) -> Result<Option<Vec<ListItem<'static>>>> {
        self.stack
            .current()
            .selected()
            .and_then(DirStackItem::as_path)
            .map_or(Ok(None), |current| -> Result<_> {
                Ok(match self.stack.path() {
                    [artist, album] => Some(
                        find_songs(client, artist, album, current)?
                            .first()
                            .context("Expected to find exactly one song")?
                            .to_preview(&state.config.symbols)
                            .collect_vec(),
                    ),
                    [artist] => Some(
                        list_titles(client, artist, current)?
                            .map(|s| s.to_list_item(&state.config.symbols, false))
                            .collect_vec(),
                    ),
                    [] => Some(
                        list_albums(client, current)?
                            .map(|s| s.to_list_item(&state.config.symbols, false))
                            .collect_vec(),
                    ),
                    _ => None,
                })
            })
    }
}
