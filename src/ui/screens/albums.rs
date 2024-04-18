use crate::{
    config::Config,
    mpd::{
        commands::{Song as MpdSong, Status},
        errors::MpdError,
        mpd_client::{Filter, MpdClient, Tag},
    },
    ui::{
        utils::dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        KeyHandleResultInternal,
    },
    utils::macros::status_info,
};

use super::{browser::DirOrSong, BrowserScreen, Screen};
use anyhow::{Context, Result};
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::{prelude::Rect, widgets::ListItem, Frame};
use strum::Display;

#[derive(Debug, Default)]
pub struct AlbumsScreen {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
}

impl Screen for AlbumsScreen {
    type Actions = AlbumsActions;

    fn render(&mut self, frame: &mut Frame, area: Rect, _status: &Status, config: &Config) -> Result<()> {
        frame.render_stateful_widget(
            Browser::new(config)
                .set_widths(&config.ui.column_widths)
                .set_border_style(config.as_border_style()),
            area,
            &mut self.stack,
        );

        Ok(())
    }

    fn before_show(&mut self, client: &mut impl MpdClient, _status: &mut Status, config: &Config) -> Result<()> {
        if self.stack().path().is_empty() {
            let result = client.list_tag(Tag::Album, None).context("Cannot list tags")?;
            self.stack = DirStack::new(result.into_iter().map(DirOrSong::Dir).collect::<Vec<_>>());
            let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
            self.stack.set_preview(preview);
        }

        Ok(())
    }

    fn handle_action(
        &mut self,
        event: KeyEvent,
        client: &mut impl MpdClient,
        _status: &mut Status,
        config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        if self.filter_input_mode {
            self.handle_filter_input(event, client, config)?;
            Ok(KeyHandleResultInternal::RenderRequested)
        } else if let Some(_action) = config.keybinds.albums.get(&event.into()) {
            Ok(KeyHandleResultInternal::SkipRender)
        } else if let Some(action) = config.keybinds.navigation.get(&event.into()) {
            self.handle_common_action(*action, client, config)
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum AlbumsActions {}

fn list_titles(client: &mut impl MpdClient, album: &str) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .list_tag(Tag::Title, Some(&[Filter::new(Tag::Album, album)]))?
        .into_iter()
        .map(DirOrSong::Song))
}

fn find_songs(client: &mut impl MpdClient, album: &str, file: &str) -> Result<Vec<MpdSong>, MpdError> {
    client.find(&[Filter::new(Tag::Title, file), Filter::new(Tag::Album, album)])
}

impl BrowserScreen<DirOrSong> for AlbumsScreen {
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
                    Filter::new(Tag::Title, item.value()),
                    Filter::new(Tag::Album, album.as_str()),
                ])?;

                status_info!("'{}' from album '{album}' added to queue", item.value());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [] => {
                client.find_add(&[Filter::new(Tag::Album, item.value())])?;

                status_info!("Album '{}' added to queue", item.value());
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
                    [album] => find_songs(client, album, current)?
                        .first()
                        .map(|v| v.to_preview(&config.ui.symbols))
                        .map(std::iter::Iterator::collect),
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
