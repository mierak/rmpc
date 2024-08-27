use crate::{
    config::{keys::ArtistsActions, Config},
    context::AppContext,
    mpd::{
        commands::Song,
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

use super::{browser::DirOrSong, BrowserScreen, Screen};
use anyhow::{anyhow, Context, Result};
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::{prelude::Rect, widgets::ListItem, Frame};

#[derive(Debug, Default)]
pub struct ArtistsScreen {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
}

impl Screen for ArtistsScreen {
    type Actions = ArtistsActions;
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
            let result = client.list_tag(Tag::Artist, None).context("Cannot list artists")?;
            self.stack = DirStack::new(
                result
                    .into_iter()
                    .map(|v| DirOrSong::Dir {
                        full_path: format!("Artists/{}{v}", self.stack().path().join("/")),
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
                let result = client.list_tag(Tag::Artist, None).context("Cannot list artists")?;
                self.stack = DirStack::new(
                    result
                        .into_iter()
                        .map(|v| DirOrSong::Dir {
                            full_path: format!("Artists/{}{v}", self.stack().path().join("/")),
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
            self.handle_filter_input(event, client, config)
        } else if let Some(_action) = config.keybinds.artists.get(&event.into()) {
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

fn list_titles(
    client: &mut impl MpdClient,
    artist: &str,
    album: &str,
) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .find(&[Filter::new(Tag::Artist, artist), Filter::new(Tag::Album, album)])?
        .into_iter()
        .map(DirOrSong::Song)
        .sorted())
}

fn list_albums(client: &mut impl MpdClient, artist: &str) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .list_tag(Tag::Album, Some(&[Filter::new(Tag::Artist, artist)]))?
        .into_iter()
        .map(|v| DirOrSong::Dir {
            full_path: format!("Artists/{artist}/{v}"),
            name: v,
        })
        .sorted())
}

fn find_songs(client: &mut impl MpdClient, artist: &str, album: &str, file: &str) -> Result<Vec<Song>, MpdError> {
    client
        .find(&[
            Filter::new(Tag::File, file),
            Filter::new(Tag::Artist, artist),
            Filter::new(Tag::Album, album),
        ])
        .map(|mut v| {
            v.sort();
            v
        })
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

    fn list_songs_in_item(&self, client: &mut impl MpdClient, item: &DirOrSong) -> Result<Vec<Song>> {
        Ok(match item {
            DirOrSong::Dir { name, full_path: _ } => match self.stack().path() {
                [artist] => client
                    .find(&[Filter::new(Tag::Album, name), Filter::new(Tag::Artist, artist)])?
                    .into_iter()
                    .map(|mut song| std::mem::take(&mut song))
                    .collect_vec(),
                [] => client
                    .find(&[Filter::new(Tag::Artist, name)])?
                    .into_iter()
                    .map(|mut song| std::mem::take(&mut song))
                    .collect_vec(),
                _ => Vec::new(),
            },
            DirOrSong::Song(song) => vec![song.clone()],
        })
    }

    fn add(&self, item: &DirOrSong, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        match self.stack.path() {
            [artist, album] => {
                client.find_add(&[
                    Filter::new(Tag::Artist, artist.as_str()),
                    Filter::new(Tag::Album, album.as_str()),
                    Filter::new(Tag::File, &item.dir_name_or_file_name()),
                ])?;

                status_info!("'{}' added to queue", item.dir_name_or_file_name());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [artist] => {
                client.find_add(&[
                    Filter::new(Tag::Artist, artist.as_str()),
                    Filter::new(Tag::Album, &item.dir_name_or_file_name()),
                ])?;

                status_info!("Album '{}' by '{artist}' added to queue", item.dir_name_or_file_name());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [] => {
                client.find_add(&[Filter::new(Tag::Artist, &item.dir_name_or_file_name())])?;

                status_info!("All songs by '{}' added to queue", item.dir_name_or_file_name());
                Ok(KeyHandleResultInternal::SkipRender)
            }
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }

    fn add_all(&self, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        match self.stack.path() {
            [artist, album] => {
                client.find_add(&[
                    Filter::new(Tag::Artist, artist.as_str()),
                    Filter::new(Tag::Album, album.as_str()),
                ])?;

                status_info!("Album '{album}' by '{artist}' added to queue");

                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [artist] => {
                client.find_add(&[Filter::new(Tag::Artist, artist.as_str())])?;

                status_info!("All albums by '{artist}' added to queue");
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [] => {
                client.add("/")?; // add the whole library
                status_info!("All songs added to queue");

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
                            .context(anyhow!(
                                "Expected to find exactly one song: artist: '{}', album: '{}', current: '{}'",
                                artist,
                                album,
                                current
                            ))?
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
