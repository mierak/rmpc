use crate::{
    config::Config,
    context::AppContext,
    mpd::{
        commands::Song as MpdSong,
        errors::MpdError,
        mpd_client::{Filter, MpdClient, Tag},
    },
    ui::{
        utils::dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        KeyHandleResultInternal,
    },
    utils::macros::{status_info, status_warn},
};

use super::{browser::DirOrSong, BrowserPane, Pane};
use anyhow::{anyhow, Context, Result};
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::{prelude::Rect, widgets::ListItem, Frame};

#[derive(Debug, Default)]
pub struct AlbumsPane {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
}

impl Pane for AlbumsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, AppContext { config, .. }: &AppContext) -> Result<()> {
        frame.render_stateful_widget(
            Browser::new(config)
                .set_widths(&config.theme.column_widths)
                .set_border_style(config.as_border_style()),
            area,
            &mut self.stack,
        );

        Ok(())
    }

    fn before_show(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        if self.stack().path().is_empty() {
            let result = client.list_tag(Tag::Album, None).context("Cannot list tags")?;
            self.stack = DirStack::new(
                result
                    .into_iter()
                    .map(|v| DirOrSong::Dir {
                        full_path: String::new(),
                        name: v,
                    })
                    .collect::<Vec<_>>(),
            );
            let preview = self
                .prepare_preview(client, context.config)
                .context("Cannot prepare preview")?;
            self.stack.set_preview(preview);
        }

        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut crate::ui::UiEvent,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<KeyHandleResultInternal> {
        match event {
            crate::ui::UiEvent::Database => {
                let result = client.list_tag(Tag::Album, None).context("Cannot list tags")?;
                self.stack = DirStack::new(
                    result
                        .into_iter()
                        .map(|v| DirOrSong::Dir {
                            full_path: String::new(),
                            name: v,
                        })
                        .collect::<Vec<_>>(),
                );
                let preview = self
                    .prepare_preview(client, context.config)
                    .context("Cannot prepare preview")?;
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
        context: &AppContext,
    ) -> Result<KeyHandleResultInternal> {
        let config = context.config;
        if self.filter_input_mode {
            self.handle_filter_input(event, client, config)?;
            Ok(KeyHandleResultInternal::RenderRequested)
        } else if let Some(_action) = config.keybinds.albums.get(&event.into()) {
            Ok(KeyHandleResultInternal::SkipRender)
        } else if let Some(action) = config.keybinds.navigation.get(&event.into()) {
            self.handle_common_action(*action, client, context)
        } else if let Some(action) = config.keybinds.global.get(&event.into()) {
            self.handle_global_action(*action, client, context)
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

fn list_titles(client: &mut impl MpdClient, album: &str) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .find(&[Filter::new(Tag::Album, album)])?
        .into_iter()
        .map(DirOrSong::Song)
        .sorted())
}

fn find_songs(client: &mut impl MpdClient, album: &str, file: &str) -> Result<Vec<MpdSong>, MpdError> {
    client
        .find(&[Filter::new(Tag::File, file), Filter::new(Tag::Album, album)])
        .map(|mut v| {
            v.sort();
            v
        })
}

impl BrowserPane<DirOrSong> for AlbumsPane {
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

    fn list_songs_in_item(&self, client: &mut impl MpdClient, item: &DirOrSong) -> Result<Vec<MpdSong>> {
        match item {
            DirOrSong::Dir { name, full_path: _ } => Ok(client.find(&[Filter::new(Tag::Album, name)])?),
            DirOrSong::Song(song) => Ok(vec![song.clone()]),
        }
    }

    fn next(&mut self, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        let Some(current) = self.stack.current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(KeyHandleResultInternal::RenderRequested);
        };

        match self.stack.path() {
            [_album] => self.add(current, client),
            [] => {
                let res = list_titles(client, current.as_path())?;
                self.stack.push(res.collect());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            _ => {
                log::error!("Unexpected nesting in Artists dir structure");
                Ok(KeyHandleResultInternal::RenderRequested)
            }
        }
    }

    fn add(&self, item: &DirOrSong, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        match self.stack.path() {
            [album] => {
                client.find_add(&[
                    Filter::new(Tag::File, &item.dir_name_or_file_name()),
                    Filter::new(Tag::Album, album.as_str()),
                ])?;

                status_info!("'{}' added to queue", item.dir_name_or_file_name());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [] => {
                client.find_add(&[Filter::new(Tag::Album, &item.dir_name_or_file_name())])?;

                status_info!("Album '{}' added to queue", &item.dir_name_or_file_name());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }

    fn add_all(&self, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        match self.stack.path() {
            [album] => {
                client.find_add(&[Filter::new(Tag::Album, album.as_str())])?;
                status_info!("Album '{}' added to queue", album);

                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [] => {
                client.add("/")?; // add the whole library
                status_info!("All albums added to queue");

                Ok(KeyHandleResultInternal::RenderRequested)
            }
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }

    fn prepare_preview(
        &mut self,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<Option<Vec<ListItem<'static>>>> {
        self.stack()
            .current()
            .selected()
            .map(DirStackItem::as_path)
            .map_or(Ok(None), |current| -> Result<_> {
                Ok(match self.stack.path() {
                    [album] => Some(
                        find_songs(client, album, current)?
                            .first()
                            .context(anyhow!(
                                "Expected to find exactly one song: album: '{}', current: '{}'",
                                album,
                                current
                            ))?
                            .to_preview(&config.theme.symbols)
                            .collect_vec(),
                    ),
                    [] => Some(
                        list_titles(client, current)?
                            .map(|v| v.to_list_item(config, false, None))
                            .collect_vec(),
                    ),
                    _ => None,
                })
            })
    }
}
