use std::cmp::Ordering;

use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::{prelude::Rect, widgets::ListItem, Frame};
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

use super::{browser::DirOrSong, BrowserScreen, Screen, SongExt};

#[derive(Debug, Default)]
pub struct DirectoriesScreen {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
}

impl Screen for DirectoriesScreen {
    type Actions = DirectoriesActions;
    fn render(
        &mut self,
        frame: &mut Frame,
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
            self.handle_filter_input(event);
            Ok(KeyHandleResultInternal::RenderRequested)
        } else if let Some(_action) = app.config.keybinds.directories.get(&event.into()) {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        } else if let Some(action) = app.config.keybinds.navigation.get(&event.into()) {
            self.handle_common_action(*action, client, app, shared)
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum DirectoriesActions {}

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

impl BrowserScreen<DirOrSong> for DirectoriesScreen {
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
        match item {
            DirOrSong::Dir(dirname) => {
                let mut next_path = self.stack.path().to_vec();
                next_path.push(dirname.clone());
                let next_path = next_path.join("/").to_string();

                client.add(&next_path)?;
                shared.status_message = Some(StatusMessage::new(
                    format!("Directory '{next_path}' added to queue"),
                    Level::Info,
                ));
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

    fn next(&mut self, client: &mut Client<'_>, shared: &mut SharedUiState) -> Result<()> {
        let Some(selected) = self.stack.current().selected() else {
            tracing::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };
        let Some(next_path) = self.stack.next_path() else {
            tracing::error!("Failed to move deeper inside dir. Next path is None");
            return Ok(());
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
        Ok(())
    }

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
