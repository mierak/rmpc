use crate::{
    config::Config,
    context::AppContext,
    mpd::{
        commands::Song,
        errors::MpdError,
        mpd_client::{Filter, MpdClient, Tag},
    },
    ui::{
        browser::BrowserPane,
        utils::dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        KeyHandleResultInternal,
    },
    utils::{
        macros::{status_info, status_warn},
        mouse_event::MouseEvent,
    },
};

use super::{browser::DirOrSong, Pane};
use anyhow::{anyhow, Context, Result};
use crossterm::event::KeyEvent;
use itertools::Itertools;
use ratatui::{
    prelude::Rect,
    widgets::{ListItem, StatefulWidget},
    Frame,
};

#[derive(Debug)]
pub enum ArtistsPaneMode {
    AlbumArtist,
    Artist,
}
#[derive(Debug)]
pub struct ArtistsPane {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
    mode: ArtistsPaneMode,
    browser: Browser<DirOrSong>,
}

impl ArtistsPane {
    pub fn new(mode: ArtistsPaneMode, context: &AppContext) -> Self {
        Self {
            mode,
            stack: DirStack::default(),
            filter_input_mode: false,
            browser: Browser::new(context.config),
        }
    }

    fn artist_tag(&self) -> Tag<'_> {
        match self.mode {
            ArtistsPaneMode::AlbumArtist => Tag::AlbumArtist,
            ArtistsPaneMode::Artist => Tag::Artist,
        }
    }

    fn list_titles(
        &self,
        client: &mut impl MpdClient,
        artist: &str,
        album: &str,
    ) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
        Ok(client
            .find(&[Filter::new(self.artist_tag(), artist), Filter::new(Tag::Album, album)])?
            .into_iter()
            .map(DirOrSong::Song)
            .sorted())
    }

    fn list_albums(
        &self,
        client: &mut impl MpdClient,
        artist: &str,
    ) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
        Ok(client
            .list_tag(Tag::Album, Some(&[Filter::new(self.artist_tag(), artist)]))?
            .into_iter()
            .map(|v| DirOrSong::Dir {
                full_path: String::new(),
                name: v,
            })
            .sorted())
    }

    fn find_songs(
        &self,
        client: &mut impl MpdClient,
        artist: &str,
        album: &str,
        file: &str,
    ) -> Result<Vec<Song>, MpdError> {
        client
            .find(&[
                Filter::new(Tag::File, file),
                Filter::new(self.artist_tag(), artist),
                Filter::new(Tag::Album, album),
            ])
            .map(|mut v| {
                v.sort();
                v
            })
    }
}

impl Pane for ArtistsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, _context: &AppContext) -> Result<()> {
        self.browser.render(area, frame.buffer_mut(), &mut self.stack);

        Ok(())
    }

    fn before_show(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        if self.stack().path().is_empty() {
            let result = client
                .list_tag(self.artist_tag(), None)
                .context("Cannot list artists")?;
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
                let result = client
                    .list_tag(self.artist_tag(), None)
                    .context("Cannot list artists")?;
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

    fn handle_mouse_event(
        &mut self,
        event: MouseEvent,
        client: &mut impl MpdClient,
        context: &mut AppContext,
    ) -> Result<KeyHandleResultInternal> {
        self.handle_mouse_action(event, client, context)
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

impl BrowserPane<DirOrSong> for ArtistsPane {
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
                [artist] => client.find(&[Filter::new(Tag::Album, name), Filter::new(self.artist_tag(), artist)])?,
                [] => client.find(&[Filter::new(self.artist_tag(), name)])?,
                _ => Vec::new(),
            },
            DirOrSong::Song(song) => vec![song.clone()],
        })
    }

    fn add(&self, item: &DirOrSong, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        match self.stack.path() {
            [artist, album] => {
                client.find_add(&[
                    Filter::new(self.artist_tag(), artist.as_str()),
                    Filter::new(Tag::Album, album.as_str()),
                    Filter::new(Tag::File, &item.dir_name_or_file_name()),
                ])?;

                status_info!("'{}' added to queue", item.dir_name_or_file_name());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [artist] => {
                client.find_add(&[
                    Filter::new(self.artist_tag(), artist.as_str()),
                    Filter::new(Tag::Album, &item.dir_name_or_file_name()),
                ])?;

                status_info!("Album '{}' by '{artist}' added to queue", item.dir_name_or_file_name());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [] => {
                client.find_add(&[Filter::new(self.artist_tag(), &item.dir_name_or_file_name())])?;

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
                    Filter::new(self.artist_tag(), artist.as_str()),
                    Filter::new(Tag::Album, album.as_str()),
                ])?;

                status_info!("Album '{album}' by '{artist}' added to queue");

                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [artist] => {
                client.find_add(&[Filter::new(self.artist_tag(), artist.as_str())])?;

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
                    .push(self.list_titles(client, artist, current.as_path())?.collect());
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            [] => {
                self.stack.push(self.list_albums(client, current.as_path())?.collect());
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
                        self.find_songs(client, artist, album, current)?
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
                        self.list_titles(client, artist, current)?
                            .map(|s| s.to_list_item_simple(config))
                            .collect_vec(),
                    ),
                    [] => Some(
                        self.list_albums(client, current)?
                            .map(|s| s.to_list_item_simple(config))
                            .collect_vec(),
                    ),
                    _ => None,
                })
            })
    }
    fn browser_areas(&self) -> [Rect; 3] {
        self.browser.areas
    }
}
