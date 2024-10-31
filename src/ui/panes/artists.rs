use crate::{
    config::Config,
    context::AppContext,
    mpd::{
        commands::Song,
        errors::MpdError,
        mpd_client::{Filter, MpdClient, Tag},
    },
    shared::{
        key_event::KeyEvent,
        macros::{status_info, status_warn},
        mouse_event::MouseEvent,
    },
    ui::{
        browser::BrowserPane,
        dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        UiEvent,
    },
};

use super::{browser::DirOrSong, Pane};
use anyhow::{anyhow, Context, Result};
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
        self.browser
            .set_filter_input_active(self.filter_input_mode)
            .render(area, frame.buffer_mut(), &mut self.stack);

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

    fn on_event(&mut self, event: &mut UiEvent, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        if let crate::ui::UiEvent::Database = event {
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
        };
        Ok(())
    }

    fn handle_mouse_event(
        &mut self,
        event: MouseEvent,
        client: &mut impl MpdClient,
        context: &mut AppContext,
    ) -> Result<()> {
        self.handle_mouse_action(event, client, context)
    }

    fn handle_action(&mut self, event: &mut KeyEvent, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        self.handle_filter_input(event, client, context)?;
        self.handle_common_action(event, client, context)?;
        self.handle_global_action(event, client, context)?;
        Ok(())
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

    fn add(&self, item: &DirOrSong, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        match self.stack.path() {
            [artist, album] => {
                client.find_add(&[
                    Filter::new(self.artist_tag(), artist.as_str()),
                    Filter::new(Tag::Album, album.as_str()),
                    Filter::new(Tag::File, &item.dir_name_or_file_name()),
                ])?;

                status_info!("'{}' added to queue", item.dir_name_or_file_name());

                context.render()?;
            }
            [artist] => {
                client.find_add(&[
                    Filter::new(self.artist_tag(), artist.as_str()),
                    Filter::new(Tag::Album, &item.dir_name_or_file_name()),
                ])?;

                status_info!("Album '{}' by '{artist}' added to queue", item.dir_name_or_file_name());

                context.render()?;
            }
            [] => {
                client.find_add(&[Filter::new(self.artist_tag(), &item.dir_name_or_file_name())])?;

                status_info!("All songs by '{}' added to queue", item.dir_name_or_file_name());
            }
            _ => {}
        };

        Ok(())
    }

    fn add_all(&self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        match self.stack.path() {
            [artist, album] => {
                client.find_add(&[
                    Filter::new(self.artist_tag(), artist.as_str()),
                    Filter::new(Tag::Album, album.as_str()),
                ])?;

                status_info!("Album '{album}' by '{artist}' added to queue");

                context.render()?;
            }
            [artist] => {
                client.find_add(&[Filter::new(self.artist_tag(), artist.as_str())])?;

                status_info!("All albums by '{artist}' added to queue");

                context.render()?;
            }
            [] => {
                client.add("/")?; // add the whole library
                status_info!("All songs added to queue");
            }
            _ => {}
        };
        Ok(())
    }

    fn next(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        let Some(current) = self.stack.current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };

        match self.stack.path() {
            [_artist, _album] => {
                self.add(current, client, context)?;
            }
            [artist] => {
                self.stack
                    .push(self.list_titles(client, artist, current.as_path())?.collect());

                context.render()?;
            }
            [] => {
                self.stack.push(self.list_albums(client, current.as_path())?.collect());
                context.render()?;
            }
            _ => {
                log::error!("Unexpected nesting in Artists dir structure");
                context.render()?;
            }
        };

        Ok(())
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
