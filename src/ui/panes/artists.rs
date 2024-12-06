use crate::{
    config::tabs::PaneType,
    context::AppContext,
    mpd::{
        client::Client,
        commands::Song,
        errors::MpdError,
        mpd_client::{Filter, MpdClient, Tag},
    },
    shared::{ext::mpd_client::MpdClientExt, key_event::KeyEvent, macros::status_info, mouse_event::MouseEvent},
    ui::{
        browser::BrowserPane,
        dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        UiEvent,
    },
    MpdQueryResult,
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

const INIT: &str = "init";
const OPEN_OR_PLAY: &str = "open_or_play";
const PREVIEW: &str = "preview";

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
        let Some(next_path) = self.stack.next_path() else {
            log::error!("Failed to move deeper inside dir. Next path is None");
            return Ok(());
        };

        match self.stack.path() {
            [_artist, _album] => {
                self.add(current, context)?;
                let queue_len = context.queue.len();
                if autoplay {
                    context.command(move |client| Ok(client.play_last(queue_len)?));
                }
            }
            [artist] => {
                let artist_tag = self.artist_tag();
                let target = self.target_pane();
                let current = current.clone();
                let artist = artist.clone();
                context
                    .query()
                    .id(OPEN_OR_PLAY)
                    .replace_id(OPEN_OR_PLAY)
                    .target(target)
                    .query(move |client| {
                        let result = Self::list_titles(client, &artist, current.as_path(), artist_tag)?.collect();
                        Ok(MpdQueryResult::DirOrSong {
                            data: result,
                            origin_path: Some(next_path),
                        })
                    });
                self.stack_mut().push(Vec::new());
                self.stack_mut().clear_preview();
                context.render()?;
            }
            [] => {
                let artist_tag = self.artist_tag();
                let current = current.clone();
                let target = self.target_pane();
                context
                    .query()
                    .id(OPEN_OR_PLAY)
                    .replace_id(OPEN_OR_PLAY)
                    .target(target)
                    .query(move |client| {
                        let result = Self::list_albums(client, current.as_path(), artist_tag)?.collect();
                        Ok(MpdQueryResult::DirOrSong {
                            data: result,
                            origin_path: Some(next_path),
                        })
                    });
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
            context
                .query()
                .id(INIT)
                .replace_id(INIT)
                .target(target)
                .query(move |client| {
                    let result = client.list_tag(artist_tag, None).context("Cannot list artists")?;
                    Ok(MpdQueryResult::LsInfo {
                        data: result.0,
                        origin_path: None,
                    })
                });

            self.initialized = true;
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, context: &AppContext) -> Result<()> {
        if let crate::ui::UiEvent::Database = event {
            let target = self.target_pane();
            let artist_tag = self.artist_tag();
            context
                .query()
                .id(INIT)
                .replace_id(INIT)
                .target(target)
                .query(move |client| {
                    let result = client.list_tag(artist_tag, None).context("Cannot list artists")?;
                    Ok(MpdQueryResult::LsInfo {
                        data: result.0,
                        origin_path: None,
                    })
                });
        };
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &AppContext) -> Result<()> {
        self.handle_mouse_action(event, context)
    }

    fn handle_action(&mut self, event: &mut KeyEvent, context: &mut AppContext) -> Result<()> {
        self.handle_filter_input(event, context)?;
        self.handle_common_action(event, context)?;
        self.handle_global_action(event, context)?;
        Ok(())
    }

    fn on_query_finished(&mut self, id: &'static str, data: MpdQueryResult, context: &AppContext) -> Result<()> {
        match (id, data) {
            (PREVIEW, MpdQueryResult::Preview { data, origin_path }) => {
                if let Some(origin_path) = origin_path {
                    if origin_path != self.stack().path() {
                        log::trace!(origin_path:?, current_path:? = self.stack().path(); "Dropping preview because it does not belong to this path");
                        return Ok(());
                    }
                }
                self.stack_mut().set_preview(data);
                context.render()?;
            }
            (OPEN_OR_PLAY, MpdQueryResult::DirOrSong { data, origin_path }) => {
                if let Some(origin_path) = origin_path {
                    if origin_path != self.stack().path() {
                        log::trace!(origin_path:?, current_path:? = self.stack().path(); "Dropping result because it does not belong to this path");
                        return Ok(());
                    }
                }
                self.stack_mut().replace(data);
                self.prepare_preview(context);
                context.render()?;
            }
            (INIT, MpdQueryResult::LsInfo { data, origin_path: _ }) => {
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

    fn list_songs_in_item(&self, item: DirOrSong) -> impl FnOnce(&mut Client<'_>) -> Result<Vec<Song>> + 'static {
        let tag = self.artist_tag();
        let path = self.stack().path().to_owned();
        move |client| {
            Ok(match item {
                DirOrSong::Dir { name, full_path: _ } => match path.as_slice() {
                    [artist] => client.find(&[Filter::new(Tag::Album, &name), Filter::new(tag, artist)])?,
                    [] => client.find(&[Filter::new(tag, &name)])?,
                    _ => Vec::new(),
                },
                DirOrSong::Song(song) => vec![song.clone()],
            })
        }
    }

    fn add(&self, item: &DirOrSong, context: &AppContext) -> Result<()> {
        match self.stack.path() {
            [artist, album] => {
                let artist_tag = self.artist_tag();
                let album = album.clone();
                let artist = artist.clone();
                let name = item.dir_name_or_file_name().into_owned();
                context.command(move |client| {
                    client.find_add(&[
                        Filter::new(artist_tag, artist.as_str()),
                        Filter::new(Tag::Album, album.as_str()),
                        Filter::new(Tag::File, &name),
                    ])?;

                    status_info!("'{name}' added to queue");
                    Ok(())
                });
            }
            [artist] => {
                let artist = artist.clone();
                let name = item.dir_name_or_file_name().into_owned();
                let artist_tag = self.artist_tag();
                context.command(move |client| {
                    client.find_add(&[Filter::new(artist_tag, artist.as_str()), Filter::new(Tag::Album, &name)])?;

                    status_info!("Album '{name}' by '{artist}' added to queue");
                    Ok(())
                });
            }
            [] => {
                let name = item.dir_name_or_file_name().into_owned();
                let artist_tag = self.artist_tag();
                context.command(move |client| {
                    client.find_add(&[Filter::new(artist_tag, &name)])?;

                    status_info!("All songs by '{name}' added to queue");
                    Ok(())
                });
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
                context.command(move |client| {
                    client.find_add(&[
                        Filter::new(artist_tag, artist.as_str()),
                        Filter::new(Tag::Album, album.as_str()),
                    ])?;
                    status_info!("Album '{album}' by '{artist}' added to queue");
                    Ok(())
                });
            }
            [artist] => {
                let artist = artist.clone();
                context.command(move |client| {
                    client.find_add(&[Filter::new(artist_tag, artist.as_str())])?;
                    status_info!("All albums by '{artist}' added to queue");
                    Ok(())
                });
            }
            [] => {
                context.command(move |client| {
                    client.add("/")?; // add the whole library
                    status_info!("All songs added to queue");
                    Ok(())
                });
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

    fn prepare_preview(&mut self, context: &AppContext) {
        let Some(current) = self.stack.current().selected().map(DirStackItem::as_path) else {
            return;
        };
        let current = current.to_owned();
        let config = context.config;
        let artist_tag = self.artist_tag();
        let target = self.target_pane();
        let origin_path = Some(self.stack().path().to_vec());

        self.stack_mut().clear_preview();
        match self.stack.path() {
            [artist, album] => {
                let artist = artist.clone();
                let album = album.clone();
                context
                    .query()
                    .id(PREVIEW)
                    .replace_id("artists_preview")
                    .target(target)
                    .query(move |client| {
                        let data = Some(
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
                        Ok(MpdQueryResult::Preview { data, origin_path })
                    });
            }
            [artist] => {
                let artist = artist.clone();
                context
                    .query()
                    .id(PREVIEW)
                    .replace_id(PREVIEW)
                    .target(target)
                    .query(move |client| {
                        let data = Some(
                            Self::list_titles(client, &artist, &current, artist_tag)?
                                .map(|s| s.to_list_item_simple(config))
                                .collect_vec(),
                        );

                        Ok(MpdQueryResult::Preview { data, origin_path })
                    });
            }
            [] => {
                context
                    .query()
                    .id(PREVIEW)
                    .replace_id(PREVIEW)
                    .target(target)
                    .query(move |client| {
                        let data = Some(
                            Self::list_albums(client, &current, artist_tag)?
                                .map(|s| s.to_list_item_simple(config))
                                .collect_vec(),
                        );
                        Ok(MpdQueryResult::Preview { data, origin_path })
                    });
            }
            _ => {}
        };
    }
    fn browser_areas(&self) -> [Rect; 3] {
        self.browser.areas
    }
}
