use anyhow::Result;
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::{prelude::Rect, widgets::ListItem, Frame};

use crate::{
    config::{keys::DirectoriesActions, Config},
    mpd::{
        commands::{lsinfo::FileOrDir, Status},
        mpd_client::{Filter, MpdClient, Tag},
    },
    ui::{
        utils::dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        KeyHandleResultInternal, 
    },
    utils::macros::{status_info, status_warn},
};

use super::{browser::DirOrSong, BrowserScreen, Screen};

#[derive(Debug, Default)]
pub struct DirectoriesScreen {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
}

impl Screen for DirectoriesScreen {
    type Actions = DirectoriesActions;
    fn render(&mut self, frame: &mut Frame, area: Rect, _status: &Status, config: &Config) -> anyhow::Result<()> {
        frame.render_stateful_widget(
            Browser::new(config)
                .set_widths(&config.theme.column_widths)
                .set_border_style(config.as_border_style()),
            area,
            &mut self.stack,
        );

        Ok(())
    }

    fn before_show(&mut self, client: &mut impl MpdClient, _status: &mut Status, config: &Config) -> Result<()> {
        if self.stack().path().is_empty() {
            self.stack = DirStack::new(
                client
                    .lsinfo(None)?
                    .into_iter()
                    .map(Into::<DirOrSong>::into)
                    .collect::<Vec<_>>(),
            );
            let preview = self.prepare_preview(client, config)?;
            self.stack.set_preview(preview);
        }

        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut crate::ui::UiEvent,
        client: &mut impl MpdClient,
        _status: &mut Status,
        config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        match event {
            crate::ui::UiEvent::Database => {
                self.stack = DirStack::new(
                    client
                        .lsinfo(None)?
                        .into_iter()
                        .map(Into::<DirOrSong>::into)
                        .collect::<Vec<_>>(),
                );
                let preview = self.prepare_preview(client, config)?;
                self.stack.set_preview(preview);

                status_warn!("The music database has been updated. The current tab has been reinitialized in the root directory to prevent inconsistent behaviours.");
                Ok(KeyHandleResultInternal::SkipRender)
            }
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }

    fn handle_action(
        &mut self,
        event: KeyEvent,
        client: &mut impl MpdClient,
        _status: &mut Status,
        config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        if self.filter_input_mode {
            self.handle_filter_input(event, client, config)
        } else if let Some(_action) = config.keybinds.directories.get(&event.into()) {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        } else if let Some(action) = config.keybinds.navigation.get(&event.into()) {
            self.handle_common_action(*action, client, config)
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
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
        config: &Config,
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
                .map(|v| v.to_list_item(config, false, None))
                .collect();
                Ok(Some(res))
            }
            Some(DirOrSong::Song(file)) => Ok(client
                .find_one(&[Filter::new(Tag::File, file)])?
                .map(|v| v.to_preview(&config.theme.symbols).collect())),
            None => Ok(None),
        }
    }
}
