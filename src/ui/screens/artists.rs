use crate::{
    config::SymbolsConfig,
    mpd::{
        client::Client,
        commands::Song as MpdSong,
        mpd_client::{Filter, MpdClient},
    },
    state::State,
    ui::{widgets::browser::Browser, KeyHandleResult, Level, SharedUiState, StatusMessage},
};

use super::{
    browser::{DirOrSong, ToListItems},
    dirstack::DirStack,
    iter::DirOrSongListItems,
    CommonAction, Screen,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::Rect, widgets::ListItem, Frame};
use tracing::instrument;

#[derive(Debug)]
pub struct ArtistsScreen {
    stack: DirStack<DirOrSong>,
    position: CurrentPosition,
    filter_input_mode: bool,
}

impl Default for ArtistsScreen {
    fn default() -> Self {
        Self {
            stack: DirStack::new(Vec::new()),
            position: CurrentPosition::Artist(Position { values: Artist }),
            filter_input_mode: false,
        }
    }
}

impl ArtistsScreen {
    async fn prepare_preview(
        &mut self,
        client: &mut Client<'_>,
        symbols: &SymbolsConfig,
    ) -> Result<Vec<ListItem<'static>>> {
        let idx = self
            .stack
            .current()
            .1
            .get_selected()
            .context("Expected an item to be selected")?;
        let current = &self.stack.current().0[idx];
        Ok(match &self.position {
            CurrentPosition::Artist(val) => val
                .fetch(client, current.to_current_value())
                .await?
                .listitems(symbols)
                .collect(),
            CurrentPosition::Album(val) => val
                .fetch(client, current.to_current_value())
                .await?
                .listitems(symbols)
                .collect(),
            CurrentPosition::Song(val) => val
                .fetch(client, current.to_current_value())
                .await?
                .first()
                .context("Expected to find exactly one song")?
                .to_listitems(symbols),
        })
    }
}

#[async_trait]
impl Screen for ArtistsScreen {
    type Actions = ArtistsActions;
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
        let result = _client.list_tag("artist", None).await.context("Cannot list artists")?;
        self.stack = DirStack::new(result.0.into_iter().map(DirOrSong::Dir).collect());
        self.position = CurrentPosition::default();
        self.stack.preview = self
            .prepare_preview(_client, &_app.config.symbols)
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
                        .prepare_preview(client, &app.config.symbols)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::UpHalf => {
                    self.stack.prev_half_viewport();
                    self.stack.preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Up => {
                    self.stack.prev();
                    self.stack.preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Down => {
                    self.stack.next();
                    self.stack.preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Bottom => {
                    self.stack.last();
                    self.prepare_preview(client, &app.config.symbols).await?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Top => {
                    self.stack.first();
                    self.prepare_preview(client, &app.config.symbols).await?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Right => {
                    let idx = self
                        .stack
                        .current()
                        .1
                        .get_selected()
                        .context("Expected an item to be selected")?;
                    let current = self.stack.current().0[idx].to_current_value().to_owned();
                    self.position = match &mut self.position {
                        CurrentPosition::Artist(val) => {
                            self.stack.push(val.fetch(client, &current).await?.collect());
                            CurrentPosition::Album(val.next(current))
                        }
                        CurrentPosition::Album(val) => {
                            self.stack.push(val.fetch(client, &current).await?.collect());
                            CurrentPosition::Song(val.next(current))
                        }
                        CurrentPosition::Song(val) => {
                            val.add_to_queue(client, &current).await?;
                            shared.status_message = Some(StatusMessage::new(
                                format!("'{}' by '{}' added to queue", current, val.values.artist),
                                Level::Info,
                            ));
                            CurrentPosition::Song(val.next())
                        }
                    };
                    self.stack.preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Left => {
                    self.stack.pop();
                    self.position = match &mut self.position {
                        CurrentPosition::Artist(val) => CurrentPosition::Artist(val.prev()),
                        CurrentPosition::Album(val) => CurrentPosition::Artist(val.prev()),
                        CurrentPosition::Song(val) => CurrentPosition::Album(val.prev()),
                    };
                    self.stack.preview = self
                        .prepare_preview(client, &app.config.symbols)
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

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum ArtistsActions {}

#[derive(Debug)]
struct Artist;
#[derive(Debug)]
struct Album {
    artist: String,
}
#[derive(Debug)]
struct Song {
    artist: String,
    album: String,
}
#[derive(Debug)]
struct Position<T> {
    values: T,
}
#[derive(Debug)]
enum CurrentPosition {
    Artist(Position<Artist>),
    Album(Position<Album>),
    Song(Position<Song>),
}

impl Default for CurrentPosition {
    fn default() -> Self {
        Self::Artist(Position { values: Artist })
    }
}

impl Position<Artist> {
    fn next(&self, artist: String) -> Position<Album> {
        Position {
            values: Album { artist },
        }
    }

    fn prev(&mut self) -> Position<Artist> {
        Position { values: Artist }
    }

    async fn fetch(&self, client: &mut Client<'_>, value: &str) -> Result<impl Iterator<Item = DirOrSong>> {
        Ok(client
            .list_tag("album", Some(&[Filter { tag: "artist", value }]))
            .await?
            .0
            .into_iter()
            .map(DirOrSong::Dir))
    }
}
impl Position<Album> {
    fn next(&mut self, album: String) -> Position<Song> {
        Position {
            values: Song {
                artist: std::mem::take(&mut self.values.artist),
                album,
            },
        }
    }

    fn prev(&mut self) -> Position<Artist> {
        Position { values: Artist }
    }

    async fn fetch(&self, client: &mut Client<'_>, value: &str) -> Result<impl Iterator<Item = DirOrSong>> {
        Ok(client
            .list_tag(
                "title",
                Some(&[
                    Filter {
                        tag: "artist",
                        value: &self.values.artist,
                    },
                    Filter { tag: "album", value },
                ]),
            )
            .await?
            .0
            .into_iter()
            .map(DirOrSong::Song))
    }
}

impl Position<Song> {
    fn next(&mut self) -> Position<Song> {
        Position {
            values: Song {
                artist: std::mem::take(&mut self.values.artist),
                album: std::mem::take(&mut self.values.album),
            },
        }
    }
    fn prev(&mut self) -> Position<Album> {
        Position {
            values: Album {
                artist: std::mem::take(&mut self.values.artist),
            },
        }
    }

    #[instrument(err)]
    async fn fetch(&self, client: &mut Client<'_>, value: &str) -> Result<Vec<MpdSong>> {
        Ok(client
            .find(&[
                Filter { tag: "title", value },
                Filter {
                    tag: "artist",
                    value: &self.values.artist,
                },
                Filter {
                    tag: "album",
                    value: &self.values.album,
                },
            ])
            .await?)
    }

    async fn add_to_queue(&self, client: &mut Client<'_>, value: &str) -> Result<()> {
        Ok(client
            .find_add(&[
                Filter { tag: "title", value },
                Filter {
                    tag: "artist",
                    value: &self.values.artist,
                },
                Filter {
                    tag: "album",
                    value: &self.values.album,
                },
            ])
            .await?)
    }
}
