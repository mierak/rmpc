use anyhow::Result;
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::{prelude::Rect, widgets::ListItem, Frame};
use strum::Display;

use crate::{
    mpd::{
        commands::lsinfo::FileOrDir,
        mpd_client::{Filter, MpdClient, Tag},
    },
    state::State,
    ui::{
        utils::dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        KeyHandleResultInternal,
    },
    utils::macros::status_info,
};

use super::{browser::DirOrSong, BrowserScreen, Screen};

#[derive(Debug, Default)]
pub struct DirectoriesScreen {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
}

impl Screen for DirectoriesScreen {
    type Actions = DirectoriesActions;
    fn render(&mut self, frame: &mut Frame, area: Rect, app: &mut crate::state::State) -> anyhow::Result<()> {
        frame.render_stateful_widget(
            Browser::new(app.config)
                .set_widths(&app.config.ui.column_widths)
                .set_border_style(app.config.as_border_style()),
            area,
            &mut self.stack,
        );

        Ok(())
    }

    fn before_show(&mut self, client: &mut impl MpdClient, app: &mut crate::state::State) -> Result<()> {
        if self.stack().path().is_empty() {
            self.stack = DirStack::new(
                client
                    .lsinfo(None)?
                    .into_iter()
                    .map(Into::<DirOrSong>::into)
                    .collect::<Vec<_>>(),
            );
            let preview = self.prepare_preview(client, app)?;
            self.stack.set_preview(preview);
        }

        Ok(())
    }

    fn handle_action(
        &mut self,
        event: KeyEvent,
        client: &mut impl MpdClient,
        app: &mut State,
    ) -> Result<KeyHandleResultInternal> {
        if self.filter_input_mode {
            self.handle_filter_input(event, client, app)
        } else if let Some(_action) = app.config.keybinds.directories.get(&event.into()) {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        } else if let Some(action) = app.config.keybinds.navigation.get(&event.into()) {
            self.handle_common_action(*action, client, app)
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum DirectoriesActions {}

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

    fn add(&self, item: &DirOrSong, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        match item {
            DirOrSong::Dir(dirname) => {
                let mut next_path = self.stack.path().to_vec();
                next_path.push(dirname.clone());
                let next_path = next_path.join("/").to_string();

                client.add(&next_path)?;
                status_info!("Directory '{next_path}' added to queue");
            }
            DirOrSong::Song(file) => {
                client.add(file)?;
                if let Ok(Some(song)) = client.find_one(&[Filter::new(Tag::File, file)]) {
                    status_info!("'{}' by '{}' added to queue", song.title_str(), song.artist_str());
                }
            }
        };
        Ok(KeyHandleResultInternal::RenderRequested)
    }

    fn next(&mut self, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        let Some(selected) = self.stack.current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(KeyHandleResultInternal::RenderRequested);
        };
        let Some(next_path) = self.stack.next_path() else {
            log::error!("Failed to move deeper inside dir. Next path is None");
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
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            t @ DirOrSong::Song(_) => self.add(t, client),
        }
    }

    fn prepare_preview(
        &mut self,
        client: &mut impl MpdClient,
        state: &State,
    ) -> Result<Option<Vec<ListItem<'static>>>> {
        match &self.stack.current().selected() {
            Some(DirOrSong::Dir(_)) => {
                let Some(next_path) = self.stack.next_path() else {
                    log::error!("Failed to move deeper inside dir. Next path is None");
                    return Ok(None);
                };
                let res: Vec<_> = match client.lsinfo(Some(&next_path.join("/").to_string())) {
                    Ok(val) => val,
                    Err(err) => {
                        log::error!(error:? = err; "Failed to get lsinfo for dir",);
                        return Ok(None);
                    }
                }
                .0
                .into_iter()
                .map(|v| match v {
                    FileOrDir::Dir(dir) => DirOrSong::Dir(dir.path),
                    FileOrDir::File(song) => {
                        DirOrSong::Song(song.title.as_ref().map_or("Untitled", |v| v.as_str()).to_owned())
                    }
                })
                .sorted()
                .map(|v| v.to_list_item(state.config, false, None))
                .collect();
                Ok(Some(res))
            }
            Some(DirOrSong::Song(file)) => Ok(client
                .find_one(&[Filter::new(Tag::File, file)])?
                .map(|v| v.to_preview(&state.config.ui.symbols).collect())),
            None => Ok(None),
        }
    }
}
