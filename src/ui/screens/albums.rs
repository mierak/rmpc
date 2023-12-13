use std::collections::BTreeSet;

use crate::{
    config::SymbolsConfig,
    mpd::{
        client::Client,
        commands::Song as MpdSong,
        errors::MpdError,
        mpd_client::Filter,
        mpd_client::{MpdClient, Tag},
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
use strum::Display;
use tracing::instrument;

#[derive(Debug, Default)]
pub struct AlbumsScreen {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
}

impl AlbumsScreen {
    #[instrument]
    async fn prepare_preview(
        &mut self,
        client: &mut Client<'_>,
        symbols: &SymbolsConfig,
    ) -> Result<Option<Vec<ListItem<'static>>>> {
        Ok(
            if let Some(Some(current)) = self.stack.current().selected().map(AsPath::as_path) {
                match self.stack.path() {
                    [album] => Some(
                        find_songs(client, album, current)
                            .await?
                            .first()
                            .context("Expected to find exactly one song")?
                            .to_listitems(symbols),
                    ),
                    [] => Some(
                        list_titles(client, current)
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
impl Screen for AlbumsScreen {
    type Actions = AlbumsActions;

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
        let result = _client.list_tag(Tag::Album, None).await.context("Cannot list tags")?;
        self.stack = DirStack::new(result.into_iter().map(DirOrSong::Dir).collect::<Vec<_>>());
        let preview = self
            .prepare_preview(_client, &_app.config.symbols)
            .await
            .context("Cannot prepare preview")?;
        self.stack.set_preview(preview);

        Ok(())
    }

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
                    };
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
        } else if let Some(action) = app.config.keybinds.albums.get(&event.into()) {
            match action {
                AlbumsActions::AddAll => {
                    if let Some(Some(current)) = self.stack.current().selected().map(AsPath::as_path) {
                        match self.stack.path() {
                            [album] => {
                                client
                                    .find_add(&[
                                        Filter {
                                            tag: Tag::Title,
                                            value: current,
                                        },
                                        Filter {
                                            tag: Tag::Album,
                                            value: album.as_str(),
                                        },
                                    ])
                                    .await?;
                                shared.status_message = Some(StatusMessage::new(
                                    format!("'{current}' from album '{album}' added to queue"),
                                    Level::Info,
                                ));
                                Ok(KeyHandleResultInternal::RenderRequested)
                            }
                            [] => {
                                client
                                    .find_add(&[Filter {
                                        tag: Tag::Album,
                                        value: current,
                                    }])
                                    .await?;
                                shared.status_message = Some(StatusMessage::new(
                                    format!("Album '{current}' added to queue"),
                                    Level::Info,
                                ));
                                Ok(KeyHandleResultInternal::RenderRequested)
                            }
                            _ => Ok(KeyHandleResultInternal::SkipRender),
                        }
                    } else {
                        Ok(KeyHandleResultInternal::SkipRender)
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
                        [album] => {
                            add_song(client, album, value).await?;
                            shared.status_message = Some(StatusMessage::new(
                                format!("'{value}' from album '{album}' added to queue"),
                                Level::Info,
                            ));
                        }
                        [] => {
                            let res = list_titles(client, value).await?;
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
                CommonAction::Select => {
                    self.stack.current_mut().toggle_mark_selected();
                    self.stack.next();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
            }
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum AlbumsActions {
    AddAll,
}

#[tracing::instrument]
async fn list_titles(client: &mut Client<'_>, album: &str) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .list_tag(
            Tag::Title,
            Some(&[Filter {
                tag: Tag::Album,
                value: album,
            }]),
        )
        .await?
        .into_iter()
        .map(DirOrSong::Song))
}

#[tracing::instrument]
async fn find_songs(client: &mut Client<'_>, album: &str, file: &str) -> Result<Vec<MpdSong>, MpdError> {
    client
        .find(&[
            Filter {
                tag: Tag::Title,
                value: file,
            },
            Filter {
                tag: Tag::Album,
                value: album,
            },
        ])
        .await
}

#[tracing::instrument]
async fn add_song(client: &mut Client<'_>, album: &str, file: &str) -> Result<(), MpdError> {
    client
        .find_add(&[
            Filter {
                tag: Tag::Title,
                value: file,
            },
            Filter {
                tag: Tag::Album,
                value: album,
            },
        ])
        .await
}
