use crate::{
    config::Config,
    mpd::{
        commands::{Song, Status},
        errors::MpdError,
        mpd_client::{Filter, MpdClient, Tag},
    },
    ui::{
        utils::dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        KeyHandleResultInternal, ToDescription,
    },
    utils::macros::{status_info, status_warn},
};

use super::{browser::DirOrSong, BrowserScreen, Screen};
use anyhow::{Context, Result};
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::{prelude::Rect, widgets::ListItem, Frame};
use strum::Display;

#[derive(Debug, Default)]
pub struct ArtistsScreen {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
}

impl Screen for ArtistsScreen {
    type Actions = ArtistsActions;
    fn render(&mut self, frame: &mut Frame, area: Rect, _status: &Status, config: &Config) -> Result<()> {
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
            let result = client.list_tag(Tag::Artist, None).context("Cannot list artists")?;
            self.stack = DirStack::new(result.into_iter().map(DirOrSong::Dir).collect::<Vec<_>>());
            let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
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
                let result = client.list_tag(Tag::Artist, None).context("Cannot list artists")?;
                self.stack = DirStack::new(result.into_iter().map(DirOrSong::Dir).collect::<Vec<_>>());
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
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
        } else if let Some(_action) = config.keybinds.artists.get(&event.into()) {
            Ok(KeyHandleResultInternal::SkipRender)
        } else if let Some(action) = config.keybinds.navigation.get(&event.into()) {
            self.handle_common_action(*action, client, config)
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum ArtistsActions {}

impl ToDescription for ArtistsActions {
    fn to_description(&self) -> &str {
        ""
    }
}

fn list_titles(
    client: &mut impl MpdClient,
    artist: &str,
    album: &str,
) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .list_tag(
            Tag::Title,
            Some(&[Filter::new(Tag::Artist, artist), Filter::new(Tag::Album, album)]),
        )?
        .into_iter()
        .map(DirOrSong::Song))
}

fn list_albums(client: &mut impl MpdClient, artist: &str) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .list_tag(Tag::Album, Some(&[Filter::new(Tag::Artist, artist)]))?
        .into_iter()
        .map(DirOrSong::Dir))
}

fn find_songs(client: &mut impl MpdClient, artist: &str, album: &str, file: &str) -> Result<Vec<Song>, MpdError> {
    client.find(&[
        Filter::new(Tag::Title, file),
        Filter::new(Tag::Artist, artist),
        Filter::new(Tag::Album, album),
    ])
}

impl BrowserScreen<DirOrSong> for ArtistsScreen {
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
        match self.stack.path() {
            [artist, album] => {
                client.find_add(&[
                    Filter::new(Tag::Artist, artist.as_str()),
                    Filter::new(Tag::Album, album.as_str()),
                    Filter::new(Tag::Title, item.value()),
                ])?;

                status_info!("'{}' by '{artist}' from album '{album}' added to queue", item.value());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [artist] => {
                client.find_add(&[
                    Filter::new(Tag::Artist, artist.as_str()),
                    Filter::new(Tag::Album, item.value()),
                ])?;

                status_info!("Album '{}' by '{artist}' added to queue", item.value());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [] => {
                client.find_add(&[Filter::new(Tag::Artist, item.value())])?;

                status_info!("All songs by '{}' added to queue", item.value());
                Ok(KeyHandleResultInternal::SkipRender)
            }
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }

    fn next(&mut self, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        let Some(current) = self.stack.current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(KeyHandleResultInternal::RenderRequested);
        };

        match self.stack.path() {
            [_artist, _album] => self.add(current, client),
            [artist] => {
                self.stack
                    .push(list_titles(client, artist, current.as_path())?.collect());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [] => {
                self.stack.push(list_albums(client, current.as_path())?.collect());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            _ => {
                log::error!("Unexpected nesting in Artists dir structure");
                Ok(KeyHandleResultInternal::RenderRequested)
            }
        }
    }

    fn prepare_preview(
        &mut self,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<Option<Vec<ListItem<'static>>>> {
        self.stack
            .current()
            .selected()
            .map(DirStackItem::as_path)
            .map_or(Ok(None), |current| -> Result<_> {
                Ok(match self.stack.path() {
                    [artist, album] => Some(
                        find_songs(client, artist, album, current)?
                            .first()
                            .context("Expected to find exactly one song")?
                            .to_preview(&config.theme.symbols)
                            .collect_vec(),
                    ),
                    [artist] => Some(
                        list_titles(client, artist, current)?
                            .map(|s| s.to_list_item(config, false, None))
                            .collect_vec(),
                    ),
                    [] => Some(
                        list_albums(client, current)?
                            .map(|s| s.to_list_item(config, false, None))
                            .collect_vec(),
                    ),
                    _ => None,
                })
            })
    }
}
