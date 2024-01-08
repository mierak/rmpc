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
        utils::dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        KeyHandleResultInternal, Level, SharedUiState, StatusMessage,
    },
};

use super::{browser::DirOrSong, CommonAction, Screen};
use anyhow::{Context, Result};
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
    fn prepare_preview(
        &mut self,
        client: &mut Client<'_>,
        symbols: &SymbolsConfig,
    ) -> Result<Option<Vec<ListItem<'static>>>> {
        Ok(
            if let Some(Some(current)) = self.stack.current().selected().map(DirStackItem::as_path) {
                match self.stack.path() {
                    [album] => Some(
                        find_songs(client, album, current)?
                            .first()
                            .context("Expected to find exactly one song")?
                            .to_preview(symbols)
                            .collect(),
                    ),
                    [] => Some(
                        list_titles(client, current)?
                            .map(|v| v.to_list_item(symbols, false))
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

impl Screen for AlbumsScreen {
    type Actions = AlbumsActions;

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
        let result = client.list_tag(Tag::Album, None).context("Cannot list tags")?;
        self.stack = DirStack::new(result.into_iter().map(DirOrSong::Dir).collect::<Vec<_>>());
        let preview = self
            .prepare_preview(client, &app.config.symbols)
            .context("Cannot prepare preview")?;
        self.stack.set_preview(preview);

        Ok(())
    }

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
                    };
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
        } else if let Some(action) = app.config.keybinds.albums.get(&event.into()) {
            match action {
                AlbumsActions::AddAll => {
                    if let Some(Some(current)) = self.stack.current().selected().map(DirStackItem::as_path) {
                        match self.stack.path() {
                            [album] => {
                                client.find_add(&[
                                    Filter {
                                        tag: Tag::Title,
                                        value: current,
                                    },
                                    Filter {
                                        tag: Tag::Album,
                                        value: album.as_str(),
                                    },
                                ])?;
                                shared.status_message = Some(StatusMessage::new(
                                    format!("'{current}' from album '{album}' added to queue"),
                                    Level::Info,
                                ));
                                Ok(KeyHandleResultInternal::RenderRequested)
                            }
                            [] => {
                                client.find_add(&[Filter {
                                    tag: Tag::Album,
                                    value: current,
                                }])?;
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
                    self.stack.current_mut().next_half_viewport();
                    let preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .context("Cannot prepare preview")?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::UpHalf => {
                    self.stack.current_mut().prev_half_viewport();
                    let preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .context("Cannot prepare preview")?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Up => {
                    self.stack.current_mut().prev();
                    let preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .context("Cannot prepare preview")?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    self.stack.current_mut().next();
                    let preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .context("Cannot prepare preview")?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Bottom => {
                    self.stack.current_mut().last();
                    self.prepare_preview(client, &app.config.symbols)?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Top => {
                    self.stack.current_mut().first();
                    self.prepare_preview(client, &app.config.symbols)?;
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
                            add_song(client, album, value)?;
                            shared.status_message = Some(StatusMessage::new(
                                format!("'{value}' from album '{album}' added to queue"),
                                Level::Info,
                            ));
                        }
                        [] => {
                            let res = list_titles(client, value)?;
                            self.stack.push(res.collect());
                        }
                        _ => tracing::error!("Unexpected nesting in Artists dir structure"),
                    }
                    let preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .context("Cannot prepare preview")?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Left => {
                    self.stack.pop();
                    let preview = self
                        .prepare_preview(client, &app.config.symbols)
                        .context("Cannot prepare preview")?;
                    self.stack.set_preview(preview);
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

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum AlbumsActions {
    AddAll,
}

#[tracing::instrument]
fn list_titles(client: &mut Client<'_>, album: &str) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .list_tag(
            Tag::Title,
            Some(&[Filter {
                tag: Tag::Album,
                value: album,
            }]),
        )?
        .into_iter()
        .map(DirOrSong::Song))
}

#[tracing::instrument]
fn find_songs(client: &mut Client<'_>, album: &str, file: &str) -> Result<Vec<MpdSong>, MpdError> {
    client.find(&[
        Filter {
            tag: Tag::Title,
            value: file,
        },
        Filter {
            tag: Tag::Album,
            value: album,
        },
    ])
}

#[tracing::instrument]
fn add_song(client: &mut Client<'_>, album: &str, file: &str) -> Result<(), MpdError> {
    client.find_add(&[
        Filter {
            tag: Tag::Title,
            value: file,
        },
        Filter {
            tag: Tag::Album,
            value: album,
        },
    ])
}
