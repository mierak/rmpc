use std::cmp::Ordering;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::{Backend, Rect},
    widgets::ListItem,
    Frame,
};
use strum::Display;
use tracing::instrument;

use crate::{
    mpd::{
        client::Client,
        commands::{lsinfo::FileOrDir, Song},
        mpd_client::{Filter, MpdClient, Tag},
    },
    state::State,
    ui::{
        utils::dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        KeyHandleResultInternal, Level, SharedUiState, StatusMessage,
    },
};

use super::{browser::DirOrSong, CommonAction, Screen, SongExt};

#[derive(Debug, Default)]
pub struct DirectoriesScreen {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
}

impl Screen for DirectoriesScreen {
    type Actions = DirectoriesActions;
    fn render<B: Backend>(
        &mut self,
        frame: &mut Frame<B>,
        area: Rect,
        app: &mut crate::state::State,
        _state: &mut SharedUiState,
    ) -> anyhow::Result<()> {
        frame.render_stateful_widget(
            Browser::new(&app.config.symbols).set_widths(&app.config.column_widths),
            area,
            &mut self.stack,
        );

        Ok(())
    }

    fn before_show(
        &mut self,
        client: &mut Client<'_>,
        app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        self.stack = DirStack::new(
            client
                .lsinfo(None)?
                .into_iter()
                .map(Into::<DirOrSong>::into)
                .collect::<Vec<_>>(),
        );
        let preview = self.prepare_preview(client, app)?;
        self.stack.set_preview(preview);

        Ok(())
    }

    #[instrument(skip_all)]
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
        } else if let Some(action) = app.config.keybinds.directories.get(&event.into()) {
            match action {
                DirectoriesActions::AddAll => {
                    match self.stack.current().selected() {
                        Some(DirOrSong::Dir(_)) => {
                            let Some(next_path) = self.stack.next_path() else {
                                tracing::error!("Failed to move deeper inside dir. Next path is None");
                                return Ok(KeyHandleResultInternal::RenderRequested);
                            };
                            let next_path = next_path.join("/").to_string();

                            client.add(&next_path)?;
                            shared.status_message = Some(StatusMessage::new(
                                format!("Directory '{next_path}' added to queue"),
                                Level::Info,
                            ));
                        }
                        Some(DirOrSong::Song(file)) => {
                            client.add(file)?;
                            if let Ok(Some(song)) = client.find_one(&[Filter::new(Tag::File, file)]) {
                                shared.status_message = Some(StatusMessage::new(
                                    format!("'{}' by '{}' added to queue", song.title_str(), song.artist_str()),
                                    Level::Info,
                                ));
                            }
                        }
                        None => {}
                    };
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
            }
        } else if let Some(action) = app.config.keybinds.navigation.get(&event.into()) {
            match action {
                CommonAction::DownHalf => {
                    self.stack.current_mut().next_half_viewport();
                    let preview = self.prepare_preview(client, app)?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::UpHalf => {
                    self.stack.current_mut().prev_half_viewport();
                    let preview = self.prepare_preview(client, app)?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Up => {
                    self.stack.current_mut().prev();
                    let preview = self.prepare_preview(client, app)?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    self.stack.current_mut().next();
                    let preview = self.prepare_preview(client, app)?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Bottom => {
                    self.stack.current_mut().last();
                    let preview = self.prepare_preview(client, app)?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Top => {
                    self.stack.current_mut().first();
                    let preview = self.prepare_preview(client, app)?;
                    self.stack.set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Right => {
                    let Some(selected) = self.stack.current().selected() else {
                        tracing::error!("Failed to move deeper inside dir. Current value is None");
                        return Ok(KeyHandleResultInternal::RenderRequested);
                    };
                    let Some(next_path) = self.stack.next_path() else {
                        tracing::error!("Failed to move deeper inside dir. Next path is None");
                        return Ok(KeyHandleResultInternal::RenderRequested);
                    };

                    match selected {
                        DirOrSong::Dir(_) => {
                            let new_current = client.lsinfo(Some(next_path.join("/").to_string().as_str()))?;
                            let res = new_current
                                .into_iter()
                                .map(|v| match v {
                                    FileOrDir::Dir(d) => DirOrSong::Dir(d.path),
                                    FileOrDir::File(s) => DirOrSong::Song(s.file),
                                })
                                .collect();
                            self.stack.push(res);

                            let preview = self.prepare_preview(client, app)?;
                            self.stack.set_preview(preview);
                        }
                        DirOrSong::Song(file) => {
                            client.add(file)?;
                            if let Ok(Some(song)) = client.find_one(&[Filter::new(Tag::File, file)]) {
                                shared.status_message = Some(StatusMessage::new(
                                    format!("'{}' by '{}' added to queue", song.title_str(), song.artist_str()),
                                    Level::Info,
                                ));
                            }
                        }
                    };
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Left => {
                    self.stack.pop();
                    let preview = self.prepare_preview(client, app)?;
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
pub enum DirectoriesActions {
    AddAll,
}

impl DirectoriesScreen {
    #[instrument(skip(client))]
    fn prepare_preview(&mut self, client: &mut Client<'_>, state: &State) -> Result<Option<Vec<ListItem<'static>>>> {
        match &self.stack.current().selected() {
            Some(DirOrSong::Dir(_)) => {
                let Some(next_path) = self.stack.next_path() else {
                    tracing::error!("Failed to move deeper inside dir. Next path is None");
                    return Ok(None);
                };
                let mut res: Vec<FileOrDir> = match client.lsinfo(Some(&next_path.join("/").to_string())) {
                    Ok(val) => val,
                    Err(err) => {
                        tracing::error!(message = "Failed to get lsinfo for dir", error = ?err);
                        return Ok(None);
                    }
                }
                .into();
                res.sort();
                Ok(Some(
                    res.into_iter()
                        .map(|v| match v {
                            FileOrDir::Dir(dir) => DirOrSong::Dir(dir.path),
                            FileOrDir::File(song) => {
                                DirOrSong::Song(song.title.as_ref().map_or("Untitled", |v| v.as_str()).to_owned())
                            }
                        })
                        .map(|v| v.to_list_item(&state.config.symbols, false))
                        .collect(),
                ))
            }
            Some(DirOrSong::Song(file)) => Ok(client
                .find_one(&[Filter::new(Tag::File, file)])?
                .map(|v| v.to_preview(&state.config.symbols).collect())),
            None => Ok(None),
        }
    }
}

impl std::cmp::Ord for FileOrDir {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (_, FileOrDir::Dir(_)) => Ordering::Greater,
            (FileOrDir::Dir(_), _) => Ordering::Less,
            (FileOrDir::File(Song { title: t1, .. }), FileOrDir::File(Song { title: t2, .. })) => t1.cmp(t2),
        }
    }
}
impl std::cmp::PartialOrd for FileOrDir {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
