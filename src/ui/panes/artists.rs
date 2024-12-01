use crate::{
    config::tabs::PaneType,
    context::AppContext,
    mpd::{
        commands::Song,
        errors::MpdError,
        mpd_client::{Filter, MpdClient, Tag},
    },
    shared::{key_event::KeyEvent, macros::status_info, mouse_event::MouseEvent},
    ui::{
        browser::BrowserPane,
        dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        UiEvent,
    },
    MpdCommandResult,
};

use super::{browser::DirOrSong, Pane};
use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use ratatui::{prelude::Rect, widgets::StatefulWidget, Frame};

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
    initialized: bool,
}

impl ArtistsPane {
    pub fn new(mode: ArtistsPaneMode, context: &AppContext) -> Self {
        Self {
            mode,
            stack: DirStack::default(),
            filter_input_mode: false,
            browser: Browser::new(context.config),
            initialized: false,
        }
    }

    fn artist_tag(&self) -> Tag {
        match self.mode {
            ArtistsPaneMode::AlbumArtist => Tag::AlbumArtist,
            ArtistsPaneMode::Artist => Tag::Artist,
        }
    }

    fn target_pane(&self) -> PaneType {
        match self.mode {
            ArtistsPaneMode::AlbumArtist => PaneType::AlbumArtists,
            ArtistsPaneMode::Artist => PaneType::Artists,
        }
    }

    fn list_titles(
        client: &mut impl MpdClient,
        artist: &str,
        album: &str,
        artist_tag: Tag,
    ) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
        Ok(client
            .find(&[Filter::new(artist_tag, artist), Filter::new(Tag::Album, album)])?
            .into_iter()
            .map(DirOrSong::Song)
            .sorted())
    }

    fn list_albums(
        client: &mut impl MpdClient,
        artist: &str,
        artist_tag: Tag,
    ) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
        Ok(client
            .list_tag(Tag::Album, Some(&[Filter::new(artist_tag, artist)]))?
            .into_iter()
            .map(|v| DirOrSong::Dir {
                full_path: String::new(),
                name: v,
            })
            .sorted())
    }

    fn find_songs(
        client: &mut impl MpdClient,
        artist: &str,
        album: &str,
        file: &str,
        artist_tag: Tag,
    ) -> Result<Vec<Song>, MpdError> {
        client
            .find(&[
                Filter::new(Tag::File, file),
                Filter::new(artist_tag, artist),
                Filter::new(Tag::Album, album),
            ])
            .map(|mut v| {
                v.sort();
                v
            })
    }

    fn open_or_play(&mut self, autoplay: bool, context: &AppContext) -> Result<()> {
        let Some(current) = self.stack.current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };

        match self.stack.path() {
            [_artist, _album] => {
                self.add(current, context)?;
                // if autoplay {
                //     client.play_last(context)?;
                // }
            }
            [artist] => {
                let artist_tag = self.artist_tag();
                let target = self.target_pane();
                let current = current.clone();
                let artist = artist.clone();
                context.query(
                    "next",
                    target,
                    Box::new(move |client| {
                        let result = Self::list_titles(client, &artist, current.as_path(), artist_tag)?.collect();
                        Ok(MpdCommandResult::DirOrSong(result))
                    }),
                );
                self.stack_mut().push(Vec::new());
                self.stack_mut().clear_preview();
                context.render()?;
            }
            [] => {
                let artist_tag = self.artist_tag();
                let current = current.clone();
                let target = self.target_pane();
                context.query(
                    "next",
                    target,
                    Box::new(move |client| {
                        let result = Self::list_albums(client, current.as_path(), artist_tag)?.collect();
                        Ok(MpdCommandResult::DirOrSong(result))
                    }),
                );
                self.stack_mut().push(Vec::new());
                self.stack_mut().clear_preview();
                context.render()?;
            }
            _ => {
                log::error!("Unexpected nesting in Artists dir structure");
            }
        };

        Ok(())
    }
}

impl Pane for ArtistsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, _context: &AppContext) -> Result<()> {
        self.browser
            .set_filter_input_active(self.filter_input_mode)
            .render(area, frame.buffer_mut(), &mut self.stack);

        Ok(())
    }

    fn before_show(&mut self, context: &AppContext) -> Result<()> {
        if !self.initialized {
            let target = self.target_pane();
            let artist_tag = self.artist_tag();
            context.query(
                "init",
                target,
                Box::new(move |client| {
                    let result = client.list_tag(artist_tag, None).context("Cannot list artists")?;
                    Ok(MpdCommandResult::LsInfo(result.0))
                }),
            );

            self.initialized = true;
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, context: &AppContext) -> Result<()> {
        if let crate::ui::UiEvent::Database = event {
            let target = self.target_pane();
            let artist_tag = self.artist_tag();
            context.query(
                "init",
                target,
                Box::new(move |client| {
                    let result = client.list_tag(artist_tag, None).context("Cannot list artists")?;
                    Ok(MpdCommandResult::LsInfo(result.0))
                }),
            );
        };
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &mut AppContext) -> Result<()> {
        self.handle_mouse_action(event, context)
    }

    fn handle_action(&mut self, event: &mut KeyEvent, context: &AppContext) -> Result<()> {
        self.handle_filter_input(event, context)?;
        self.handle_common_action(event, context)?;
        self.handle_global_action(event, context)?;
        Ok(())
    }

    fn on_query_finished(&mut self, id: &'static str, data: MpdCommandResult, context: &mut AppContext) -> Result<()> {
        match data {
            MpdCommandResult::Preview(vec) => {
                self.stack_mut().set_preview(vec);
                context.render()?;
            }
            MpdCommandResult::DirOrSong(data) => {
                self.stack_mut().replace(data);
                self.prepare_preview(context);
                context.render()?;
            }
            MpdCommandResult::LsInfo(data) => {
                self.stack = DirStack::new(
                    data.into_iter()
                        .map(|v| DirOrSong::Dir {
                            full_path: String::new(),
                            name: v,
                        })
                        .collect::<Vec<_>>(),
                );
                self.prepare_preview(context);
            }
            _ => {}
        };
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

    fn list_songs_in_item(client: &mut impl MpdClient, item: &DirOrSong) -> Result<Vec<Song>> {
        Ok(Vec::new())
        // Ok(match item {
        //     DirOrSong::Dir { name, full_path: _ } => match self.stack().path() {
        //         [artist] => client.find(&[Filter::new(Tag::Album, &name), Filter::new(self.artist_tag(), artist)])?,
        //         [] => client.find(&[Filter::new(self.artist_tag(), &name)])?,
        //         _ => Vec::new(),
        //     },
        //     DirOrSong::Song(song) => vec![song.clone()],
        // })
    }

    fn add(&self, item: &DirOrSong, context: &AppContext) -> Result<()> {
        match self.stack.path() {
            [artist, album] => {
                let artist_tag = self.artist_tag();
                let album = album.clone();
                let artist = artist.clone();
                let name = item.dir_name_or_file_name().into_owned();
                context.command(Box::new(move |client| {
                    client.find_add(&[
                        Filter::new(artist_tag, artist.as_str()),
                        Filter::new(Tag::Album, album.as_str()),
                        Filter::new(Tag::File, &name),
                    ])?;

                    status_info!("'{name}' added to queue");
                    Ok(())
                }));
            }
            [artist] => {
                let artist = artist.clone();
                let name = item.dir_name_or_file_name().into_owned();
                let artist_tag = self.artist_tag();
                context.command(Box::new(move |client| {
                    client.find_add(&[Filter::new(artist_tag, artist.as_str()), Filter::new(Tag::Album, &name)])?;

                    status_info!("Album '{name}' by '{artist}' added to queue");
                    Ok(())
                }));
            }
            [] => {
                let name = item.dir_name_or_file_name().into_owned();
                let artist_tag = self.artist_tag();
                context.command(Box::new(move |client| {
                    client.find_add(&[Filter::new(artist_tag, &name)])?;

                    status_info!("All songs by '{name}' added to queue");
                    Ok(())
                }));
            }
            _ => {}
        };

        Ok(())
    }

    fn add_all(&self, context: &AppContext) -> Result<()> {
        let artist_tag = self.artist_tag();
        match self.stack.path() {
            [artist, album] => {
                let artist = artist.clone();
                let album = album.clone();
                context.command(Box::new(move |client| {
                    client.find_add(&[
                        Filter::new(artist_tag, artist.as_str()),
                        Filter::new(Tag::Album, album.as_str()),
                    ])?;
                    status_info!("Album '{album}' by '{artist}' added to queue");
                    Ok(())
                }));
            }
            [artist] => {
                let artist = artist.clone();
                context.command(Box::new(move |client| {
                    client.find_add(&[Filter::new(artist_tag, artist.as_str())])?;
                    status_info!("All albums by '{artist}' added to queue");
                    Ok(())
                }));
            }
            [] => {
                context.command(Box::new(move |client| {
                    client.add("/")?; // add the whole library
                    status_info!("All songs added to queue");
                    Ok(())
                }));
            }
            _ => {}
        };
        Ok(())
    }

    fn open(&mut self, context: &AppContext) -> Result<()> {
        self.open_or_play(true, context)
    }

    fn next(&mut self, context: &AppContext) -> Result<()> {
        self.open_or_play(false, context)
    }

    fn prepare_preview(&self, context: &AppContext) {
        let Some(current) = self.stack.current().selected().map(DirStackItem::as_path) else {
            return;
        };
        let current = current.to_owned();
        let config = context.config;
        let artist_tag = self.artist_tag();
        let target = self.target_pane();

        match self.stack.path() {
            [artist, album] => {
                let artist = artist.clone();
                let album = album.clone();
                context.query(
                    "preview",
                    target,
                    Box::new(move |client| {
                        let result = Some(
                            Self::find_songs(client, &artist, &album, &current, artist_tag)?
                                .first()
                                .context(anyhow!(
                                    "Expected to find exactly one song: artist: '{}', album: '{}', current: '{}'",
                                    artist,
                                    album,
                                    current
                                ))?
                                .to_preview(&config.theme.symbols)
                                .collect_vec(),
                        );
                        Ok(MpdCommandResult::Preview(result))
                    }),
                );
            }
            [artist] => {
                let artist = artist.clone();
                context.query(
                    "preview",
                    target,
                    Box::new(move |client| {
                        let result = Some(
                            Self::list_titles(client, &artist, &current, artist_tag)?
                                .map(|s| s.to_list_item_simple(config))
                                .collect_vec(),
                        );

                        Ok(MpdCommandResult::Preview(result))
                    }),
                );
            }
            [] => context.query(
                "preview",
                target,
                Box::new(move |client| {
                    let result = Some(
                        Self::list_albums(client, &current, artist_tag)?
                            .map(|s| s.to_list_item_simple(config))
                            .collect_vec(),
                    );
                    Ok(MpdCommandResult::Preview(result))
                }),
            ),
            _ => {}
        };
    }
    fn browser_areas(&self) -> [Rect; 3] {
        self.browser.areas
    }
}
