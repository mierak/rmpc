use std::collections::BTreeSet;

use crate::{
    config::SymbolsConfig,
    mpd::{
        client::Client,
        commands::Song,
        errors::MpdError,
        mpd_client::{Filter, MpdClient, Tag},
    },
    state::State,
    ui::{
        utils::dirstack::{AsPath, DirStack},
        widgets::browser::Browser,
        KeyHandleResultInternal, Level, SharedUiState, StatusMessage,
    },
};

use super::{
    browser::{DirOrSong, ToListItems},
    iter::DirOrSongListItems,
    CommonAction, Screen,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::Rect, widgets::ListItem, Frame};
use tracing::instrument;

#[derive(Debug, Default)]
pub struct ArtistsScreen {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
}

impl ArtistsScreen {
    async fn prepare_preview(
        &mut self,
        client: &mut Client<'_>,
        symbols: &SymbolsConfig,
    ) -> Result<Option<Vec<ListItem<'static>>>> {
        Ok(
            if let Some(Some(current)) = self.stack.current().selected().map(AsPath::as_path) {
                match self.stack.path() {
                    [artist, album] => Some(
                        find_songs(client, artist, album, current)
                            .await?
                            .first()
                            .context("Expected to find exactly one song")?
                            .to_listitems(symbols),
                    ),
                    [artist] => Some(
                        list_titles(client, artist, current)
                            .await?
                            .listitems(symbols, &BTreeSet::default())
                            .collect(),
                    ),
                    [] => Some(
                        list_albums(client, current)
                            .await?
                            .listitems(symbols, &BTreeSet::default())
                            .collect(),
                    ),
                    _ => None,
                }
            } else {
                None
            },
        )
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
        let prev = self.stack.previous();
        let prev: Vec<_> = prev
            .items
            .iter()
            .cloned()
            .listitems(&app.config.symbols, prev.state.get_marked())
            .collect();
        let current = self.stack.current();
        let current: Vec<_> = current
            .items
            .iter()
            .cloned()
            .listitems(&app.config.symbols, current.state.get_marked())
            .collect();
        let preview = self.stack.preview();
        let w = Browser::new()
            .widths(&app.config.column_widths)
            .previous_items(&prev)
            .current_items(&current)
            .preview(preview.cloned());
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
        let result = _client
            .list_tag(Tag::Artist, None)
            .await
            .context("Cannot list artists")?;
        self.stack = DirStack::new(result.into_iter().map(DirOrSong::Dir).collect::<Vec<_>>());
        let preview = self
            .prepare_preview(_client, &_app.config.symbols)
            .await
            .context("Cannot prepare preview")?;
        self.stack.set_preview(preview);

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
                    self.stack.jump_next_matching();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Esc => {
                    self.filter_input_mode = false;
                    self.stack.filter = None;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                _ => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else if let Some(action) = app.config.keybinds.artists.get(&event.into()) {
            match action {
                ArtistsActions::AddAll => {
                    if let Some(Some(current)) = self.stack.current().selected().map(AsPath::as_path) {
                        match self.stack.path() {
                            [artist, album] => {
                                client
                                    .find_add(&[
                                        Filter {
                                            tag: Tag::Artist,
                                            value: artist.as_str(),
                                        },
                                        Filter {
                                            tag: Tag::Album,
                                            value: album.as_str(),
                                        },
                                        Filter {
                                            tag: Tag::Title,
                                            value: current,
                                        },
                                    ])
                                    .await?;
                                shared.status_message = Some(StatusMessage::new(
                                    format!("'{current}' by '{artist}' from album '{album}' added to queue"),
                                    Level::Info,
                                ));
                                Ok(KeyHandleResultInternal::SkipRender)
                            }
                            [artist] => {
                                client
                                    .find_add(&[
                                        Filter {
                                            tag: Tag::Artist,
                                            value: artist.as_str(),
                                        },
                                        Filter {
                                            tag: Tag::Album,
                                            value: current,
                                        },
                                    ])
                                    .await?;
                                shared.status_message = Some(StatusMessage::new(
                                    format!("Album '{current}' by '{artist}' added to queue"),
                                    Level::Info,
                                ));
                                Ok(KeyHandleResultInternal::SkipRender)
                            }
                            [] => {
                                client
                                    .find_add(&[Filter {
                                        tag: Tag::Artist,
                                        value: current,
                                    }])
                                    .await?;
                                shared.status_message = Some(StatusMessage::new(
                                    format!("All songs by '{current}' added to queue"),
                                    Level::Info,
                                ));
                                Ok(KeyHandleResultInternal::SkipRender)
                            }
                            _ => Ok(KeyHandleResultInternal::SkipRender),
                        }
                    } else {
                        Ok(KeyHandleResultInternal::RenderRequested)
                    }
                }
            }
        } else if let Some(action) = app.config.keybinds.navigation.get(&event.into()) {
            match action {
                CommonAction::DownHalf => {
                    self.stack.next_half_viewport();
                    let preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .await
                        .context("Cannot prepare preview")?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::UpHalf => {
                    self.stack.prev_half_viewport();
                    let preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .await
                        .context("Cannot prepare preview")?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Up => {
                    self.stack.prev();
                    let preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .await
                        .context("Cannot prepare preview")?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    self.stack.next();
                    let preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .await
                        .context("Cannot prepare preview")?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Bottom => {
                    self.stack.last();
                    self.prepare_preview(client, &app.config.symbols).await?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Top => {
                    self.stack.first();
                    self.prepare_preview(client, &app.config.symbols).await?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Right => {
                    let Some(current) = self.stack.current().selected() else {
                        tracing::error!("Failed to move deeper inside dir. Current value is None");
                        return Ok(KeyHandleResultInternal::RenderRequested);
                    };
                    let Some(value) = current.as_path() else {
                        tracing::error!("Failed to move deeper inside dir. Current value is None");
                        return Ok(KeyHandleResultInternal::RenderRequested);
                    };

                    match self.stack.path() {
                        [artist, album] => {
                            add_song(client, artist, album, value).await?;
                            shared.status_message = Some(StatusMessage::new(
                                format!("'{value}' by '{artist}' added to queue"),
                                Level::Info,
                            ));
                        }
                        [artist] => {
                            let res = list_titles(client, artist, value).await?;
                            self.stack.push(res.collect());
                        }
                        [] => {
                            let res = list_albums(client, value).await?;
                            self.stack.push(res.collect());
                        }
                        _ => tracing::error!("Unexpected nesting in Artists dir structure"),
                    }
                    let preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .await
                        .context("Cannot prepare preview")?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Left => {
                    self.stack.pop();
                    let preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .await
                        .context("Cannot prepare preview")?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::EnterSearch => {
                    self.filter_input_mode = true;
                    self.stack.filter = Some(String::new());
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::NextResult => {
                    self.stack.jump_next_matching();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::PreviousResult => {
                    self.stack.jump_previous_matching();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Select => Ok(KeyHandleResultInternal::RenderRequested),
            }
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum ArtistsActions {
    AddAll,
}

#[tracing::instrument]
async fn list_titles(
    client: &mut Client<'_>,
    artist: &str,
    album: &str,
) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .list_tag(
            Tag::Title,
            Some(&[
                Filter {
                    tag: Tag::Artist,
                    value: artist,
                },
                Filter {
                    tag: Tag::Album,
                    value: album,
                },
            ]),
        )
        .await?
        .into_iter()
        .map(DirOrSong::Song))
}

#[tracing::instrument]
async fn list_albums(client: &mut Client<'_>, artist: &str) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .list_tag(
            Tag::Album,
            Some(&[Filter {
                tag: Tag::Artist,
                value: artist,
            }]),
        )
        .await?
        .into_iter()
        .map(DirOrSong::Dir))
}

#[tracing::instrument]
async fn find_songs(client: &mut Client<'_>, artist: &str, album: &str, file: &str) -> Result<Vec<Song>, MpdError> {
    client
        .find(&[
            Filter {
                tag: Tag::Title,
                value: file,
            },
            Filter {
                tag: Tag::Artist,
                value: artist,
            },
            Filter {
                tag: Tag::Album,
                value: album,
            },
        ])
        .await
}

#[tracing::instrument]
async fn add_song(client: &mut Client<'_>, artist: &str, album: &str, title: &str) -> Result<(), MpdError> {
    client
        .find_add(&[
            Filter {
                tag: Tag::Title,
                value: title,
            },
            Filter {
                tag: Tag::Artist,
                value: artist,
            },
            Filter {
                tag: Tag::Album,
                value: album,
            },
        ])
        .await
}
