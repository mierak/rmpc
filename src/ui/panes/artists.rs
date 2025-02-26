use std::{cmp::Ordering, collections::HashMap};

use anyhow::{Context, Result};
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect};

use super::{Pane, browser::DirOrSong};
use crate::{
    MpdQueryResult,
    config::{
        artists::{AlbumDisplayMode, AlbumSortMode},
        tabs::PaneTypeDiscriminants,
    },
    context::AppContext,
    mpd::{
        client::Client,
        commands::Song,
        mpd_client::{Filter, MpdClient, Tag},
    },
    shared::{
        ext::mpd_client::MpdClientExt,
        key_event::KeyEvent,
        macros::status_info,
        mouse_event::MouseEvent,
        mpd_query::PreviewGroup,
    },
    ui::{
        UiEvent,
        browser::BrowserPane,
        dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
    },
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
    initialized: bool,
    cache: ArtistsCache,
}

const INIT: &str = "init";
const OPEN_OR_PLAY: &str = "open_or_play";
const PREVIEW: &str = "preview";

#[derive(Debug, Default)]
struct ArtistsCache(HashMap<String, CachedArtist>);

#[derive(Debug, Default)]
struct CachedArtist(Vec<CachedAlbum>);

#[derive(Debug, Default)]
struct CachedAlbum {
    name: String,
    original_name: String,
    songs: Vec<Song>,
}

impl ArtistsPane {
    pub fn new(mode: ArtistsPaneMode, _context: &AppContext) -> Self {
        Self {
            mode,
            stack: DirStack::default(),
            filter_input_mode: false,
            browser: Browser::new(),
            initialized: false,
            cache: ArtistsCache::default(),
        }
    }

    fn artist_tag(&self) -> Tag {
        match self.mode {
            ArtistsPaneMode::AlbumArtist => Tag::AlbumArtist,
            ArtistsPaneMode::Artist => Tag::Artist,
        }
    }

    fn target_pane(&self) -> PaneTypeDiscriminants {
        match self.mode {
            ArtistsPaneMode::AlbumArtist => PaneTypeDiscriminants::AlbumArtists,
            ArtistsPaneMode::Artist => PaneTypeDiscriminants::Artists,
        }
    }

    fn open_or_play(&mut self, autoplay: bool, context: &AppContext) -> Result<()> {
        let Some(current) = self.stack.current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };
        let Some(_next_path) = self.stack.next_path() else {
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
                let Some(albums) = self.cache.0.get(artist) else {
                    return Ok(());
                };
                let Some(CachedAlbum { songs, .. }) =
                    albums.0.iter().find(|album| album.name == current.as_path())
                else {
                    return Ok(());
                };
                let songs =
                    songs.iter().map(|song| DirOrSong::Song(song.clone())).collect::<Vec<_>>();

                self.stack_mut().push(songs);
                context.render()?;
            }
            [] => {
                let current = current.as_path().to_owned();
                if let Some(albums) = self.cache.0.get(&current) {
                    let albums = albums
                        .0
                        .iter()
                        .map(|CachedAlbum { name, .. }| DirOrSong::name_only(name.to_owned()))
                        .collect();
                    self.stack_mut().push(albums);
                } else {
                    let artist_tag = self.artist_tag();
                    let target = self.target_pane();
                    context.query().id(OPEN_OR_PLAY).replace_id(OPEN_OR_PLAY).target(target).query(
                        move |client| {
                            let all_songs: Vec<Song> =
                                client.find(&[Filter::new(artist_tag, &current)])?;
                            Ok(MpdQueryResult::SongsList {
                                data: all_songs,
                                origin_path: Some(vec![current]),
                            })
                        },
                    );
                    self.stack_mut().push(Vec::new());
                    self.stack_mut().clear_preview();
                    context.render()?;
                }
            }
            _ => {
                log::error!("Unexpected nesting in Artists dir structure");
            }
        };

        Ok(())
    }

    fn process_songs(
        &mut self,
        artist: String,
        data: Vec<Song>,
        context: &AppContext,
    ) -> &CachedArtist {
        let display_mode = context.config.artists.album_display_mode;
        let sort_mode = context.config.artists.album_sort_by;

        let cached_artist = self.cache.0.entry(artist).or_default();

        let albums = data
            .into_iter()
            .into_group_map_by(|song| {
                let album = song.album().map_or("<no album>", |v| v.as_str());
                let song_date = song.metadata.get("date").map_or("<no date>", |v| v.as_str());
                (album.to_string(), song_date.to_string(), song.album().cloned())
            })
            .into_iter()
            .sorted_by(|((album_a, date_a, _), _), ((album_b, date_b, _), _)| match sort_mode {
                AlbumSortMode::Name => match album_a.cmp(album_b) {
                    Ordering::Equal => date_a.cmp(date_b),
                    ordering => ordering,
                },
                AlbumSortMode::Date => date_a.cmp(date_b),
            })
            .map(|((album, date, original_name), songs)| CachedAlbum {
                name: match display_mode {
                    AlbumDisplayMode::SplitByDate => {
                        format!("({date}) {album}")
                    }
                    AlbumDisplayMode::NameOnly => album.to_string(),
                },
                original_name: original_name.unwrap_or_else(String::new),
                songs,
            })
            .fold(Vec::new(), |mut acc, album| {
                match display_mode {
                    AlbumDisplayMode::SplitByDate => {
                        acc.push(album);
                    }
                    AlbumDisplayMode::NameOnly => {
                        if let Some(cached_album) =
                            acc.iter_mut().find(|cached_album| cached_album.name == album.name)
                        {
                            cached_album.songs.extend(album.songs);
                        } else {
                            acc.push(album);
                        }
                    }
                };
                acc
            });

        cached_artist.0 = albums;

        cached_artist
    }
}

impl Pane for ArtistsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> Result<()> {
        self.browser.set_filter_input_active(self.filter_input_mode).render(
            area,
            frame.buffer_mut(),
            &mut self.stack,
            &context.config,
        );

        Ok(())
    }

    fn before_show(&mut self, context: &AppContext) -> Result<()> {
        if !self.initialized {
            let target = self.target_pane();
            let artist_tag = self.artist_tag();
            context.query().id(INIT).replace_id(INIT).target(target).query(move |client| {
                let result = client.list_tag(artist_tag, None).context("Cannot list artists")?;
                Ok(MpdQueryResult::LsInfo { data: result.0, origin_path: None })
            });

            self.initialized = true;
        }

        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut UiEvent,
        _is_visible: bool,
        context: &AppContext,
    ) -> Result<()> {
        match event {
            UiEvent::Database => {
                let target = self.target_pane();
                let artist_tag = self.artist_tag();
                self.cache = ArtistsCache::default();
                context.query().id(INIT).replace_id(INIT).target(target).query(move |client| {
                    let result =
                        client.list_tag(artist_tag, None).context("Cannot list artists")?;
                    Ok(MpdQueryResult::LsInfo { data: result.0, origin_path: None })
                });
            }
            UiEvent::Reconnected => {
                self.initialized = false;
                self.before_show(context)?;
            }
            _ => {}
        }
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

    fn on_query_finished(
        &mut self,
        id: &'static str,
        data: MpdQueryResult,
        _is_visible: bool,
        context: &AppContext,
    ) -> Result<()> {
        match (id, data) {
            (PREVIEW, MpdQueryResult::SongsList { data, origin_path }) => {
                let Some(artist) = origin_path.and_then(|mut v| v.first_mut().map(std::mem::take))
                else {
                    return Ok(());
                };

                let current_item_path = self.stack().current().selected().map(|c| c.as_path());
                // We still want to cache the result to avoid refetch later, but
                // do not rerender current state because rmpc is
                // already on a different item
                let cache_only = if current_item_path == Some(&artist) {
                    false
                } else {
                    log::trace!(artist:?, current_item_path:?; "Dropping preview because it does not belong to this path");
                    true
                };

                let cached_artist = self.process_songs(artist, data, context);

                if cache_only {
                    return Ok(());
                }

                let preview = vec![PreviewGroup::from(
                    None,
                    cached_artist
                        .0
                        .iter()
                        .map(|album| {
                            DirOrSong::name_only(album.name.clone())
                                .to_list_item_simple(&context.config)
                        })
                        .collect(),
                )];
                self.stack.set_preview(Some(preview));
                context.render()?;
            }
            (OPEN_OR_PLAY, MpdQueryResult::SongsList { data, origin_path }) => {
                let Some(artist) = origin_path.and_then(|mut v| v.first_mut().map(std::mem::take))
                else {
                    return Ok(());
                };

                // We still want to cache the result to avoid refetch later, but
                // do not rerender current state because rmpc is
                // already on a different item
                let cache_only = if self.stack().path().first() == Some(&artist) {
                    false
                } else {
                    log::trace!(artist:?, current_path:? = self.stack().path(); "Dropping result because it does not belong to this path");
                    true
                };

                if cache_only {
                    return Ok(());
                }

                let cached_artist = self.process_songs(artist, data, context);

                let albums = cached_artist
                    .0
                    .iter()
                    .map(|CachedAlbum { name, .. }| DirOrSong::name_only(name.to_owned()))
                    .collect();
                self.stack.replace(albums);
                self.prepare_preview(context)?;
                context.render()?;
            }
            (INIT, MpdQueryResult::LsInfo { data, origin_path: _ }) => {
                self.stack =
                    DirStack::new(data.into_iter().map(DirOrSong::name_only).collect_vec());
                self.prepare_preview(context)?;
                context.render()?;
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

    fn list_songs_in_item(
        &self,
        item: DirOrSong,
    ) -> impl FnOnce(&mut Client<'_>) -> Result<Vec<Song>> + 'static {
        let tag = self.artist_tag();
        let path = self.stack().path().to_owned();
        let album_name = match (self.stack().path(), &item) {
            ([artist], DirOrSong::Dir { name, .. }) => self
                .cache
                .0
                .get(artist)
                .and_then(|albums| {
                    albums.0.iter().find(|a| &a.name == name).map(|a| a.original_name.clone())
                })
                .unwrap_or_default(),
            _ => String::new(),
        };

        move |client| {
            Ok(match item {
                DirOrSong::Dir { name, full_path: _ } => match path.as_slice() {
                    [artist] => client
                        .find(&[Filter::new(Tag::Album, &album_name), Filter::new(tag, artist)])?,
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
                let artist = artist.clone();
                let name = item.dir_name_or_file_name().into_owned();

                let Some(albums) = self.cache.0.get(&artist) else {
                    return Ok(());
                };

                let Some(original_name) =
                    albums.0.iter().find(|a| &a.name == album).map(|a| a.original_name.clone())
                else {
                    return Ok(());
                };

                context.command(move |client| {
                    client.find_add(&[
                        Filter::new(artist_tag, artist.as_str()),
                        Filter::new(Tag::Album, original_name.as_str()),
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

                let Some(albums) = self.cache.0.get(&artist) else {
                    return Ok(());
                };

                let Some(original_name) =
                    albums.0.iter().find(|a| a.name == name).map(|a| a.original_name.clone())
                else {
                    return Ok(());
                };

                context.command(move |client| {
                    client.find_add(&[
                        Filter::new(artist_tag, artist.as_str()),
                        Filter::new(Tag::Album, &original_name),
                    ])?;

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
                let Some(albums) = self.cache.0.get(&artist) else {
                    return Ok(());
                };

                let Some(original_name) =
                    albums.0.iter().find(|a| &a.name == album).map(|a| a.original_name.clone())
                else {
                    return Ok(());
                };

                context.command(move |client| {
                    client.find_add(&[
                        Filter::new(artist_tag, artist.as_str()),
                        Filter::new(Tag::Album, original_name.as_str()),
                    ])?;
                    status_info!("Album '{original_name}' by '{artist}' added to queue");
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

    fn prepare_preview(&mut self, context: &AppContext) -> Result<()> {
        let Some(current) = self.stack.current().selected().map(DirStackItem::as_path) else {
            return Ok(());
        };
        let current = current.to_owned();

        self.stack_mut().clear_preview();
        match self.stack.path() {
            [artist, album] => {
                let Some(albums) = self.cache.0.get(artist) else {
                    return Ok(());
                };
                let Some(CachedAlbum { songs, .. }) = albums.0.iter().find(|a| &a.name == album)
                else {
                    return Ok(());
                };
                let song =
                    songs.iter().find(|song| song.file == current).map(|song| song.to_preview());
                self.stack_mut().set_preview(song);
                context.render()?;
            }
            [artist] => {
                let Some(albums) = self.cache.0.get(artist) else {
                    return Ok(());
                };
                let Some(CachedAlbum { songs, .. }) =
                    albums.0.iter().find(|album| album.name == current)
                else {
                    return Ok(());
                };
                let songs = vec![PreviewGroup::from(
                    None,
                    songs
                        .iter()
                        .map(|song| song.to_list_item_simple(&context.config))
                        .collect_vec(),
                )];
                self.stack_mut().set_preview(Some(songs));
                context.render()?;
            }
            [] => {
                if let Some(albums) = self.cache.0.get(&current) {
                    self.stack.set_preview(Some(vec![PreviewGroup::from(
                        None,
                        albums
                            .0
                            .iter()
                            .map(|CachedAlbum { name, .. }| {
                                DirOrSong::name_only(name.to_owned())
                                    .to_list_item_simple(&context.config)
                            })
                            .collect(),
                    )]));
                    context.render()?;
                } else {
                    let artist_tag = self.artist_tag();
                    let target = self.target_pane();
                    context.query().id(PREVIEW).replace_id(PREVIEW).target(target).query(
                        move |client| {
                            let all_songs: Vec<Song> =
                                client.find(&[Filter::new(artist_tag, &current)])?;
                            Ok(MpdQueryResult::SongsList {
                                data: all_songs,
                                origin_path: Some(vec![current]),
                            })
                        },
                    );
                }
            }
            _ => {}
        };
        Ok(())
    }

    fn browser_areas(&self) -> [Rect; 3] {
        self.browser.areas
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        config::Config,
        tests::fixtures::{app_context, config},
    };

    fn song(
        album: impl Into<String> + std::fmt::Debug,
        date: impl Into<String> + std::fmt::Debug,
    ) -> Song {
        Song {
            id: 0,
            file: format!("{date:?} {album:?}"),
            duration: None,
            metadata: HashMap::from([
                ("album".to_string(), album.into()),
                ("date".to_string(), date.into()),
            ]),
            stickers: None,
        }
    }

    #[rstest]
    fn albums_no_date_sort_name(mut app_context: AppContext, mut config: Config) {
        config.artists.album_display_mode = AlbumDisplayMode::NameOnly;
        config.artists.album_sort_by = AlbumSortMode::Name;
        app_context.config = std::sync::Arc::new(config);
        let mut pane = ArtistsPane::new(ArtistsPaneMode::Artist, &app_context);
        let artist = String::from("artist");
        let songs = vec![
            song("album_a", "2020"),
            song("album_b", "2022"),
            song("album_a", "2021"),
            song("album_b", "2022"),
        ];

        let CachedArtist(result) = pane.process_songs(artist, songs, &app_context);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "album_a");
        assert_eq!(result[1].name, "album_b");
    }

    #[rstest]
    fn albums_split_date_sort_name(mut app_context: AppContext, mut config: Config) {
        config.artists.album_display_mode = AlbumDisplayMode::SplitByDate;
        config.artists.album_sort_by = AlbumSortMode::Name;
        app_context.config = std::sync::Arc::new(config);
        let mut pane = ArtistsPane::new(ArtistsPaneMode::Artist, &app_context);
        let artist = String::from("artist");
        let songs = vec![
            song("album_a", "2020"),
            song("album_b", "2022"),
            song("album_a", "2021"),
            song("album_b", "2022"),
        ];

        let CachedArtist(result) = pane.process_songs(artist, songs, &app_context);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].name, "(2020) album_a");
        assert_eq!(result[1].name, "(2021) album_a");
        assert_eq!(result[2].name, "(2022) album_b");
    }

    #[rstest]
    fn albums_split_date_sort_date(mut app_context: AppContext, mut config: Config) {
        config.artists.album_display_mode = AlbumDisplayMode::SplitByDate;
        config.artists.album_sort_by = AlbumSortMode::Date;
        app_context.config = std::sync::Arc::new(config);
        let mut pane = ArtistsPane::new(ArtistsPaneMode::Artist, &app_context);
        let artist = String::from("artist");
        let songs = vec![
            song("album_a", "2020"),
            song("album_b", "2019"),
            song("album_a", "2021"),
            song("album_b", "2019"),
        ];

        let CachedArtist(result) = pane.process_songs(artist, songs, &app_context);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].name, "(2019) album_b");
        assert_eq!(result[1].name, "(2020) album_a");
        assert_eq!(result[2].name, "(2021) album_a");
    }

    #[rstest]
    fn albums_no_date_sort_date(mut app_context: AppContext, mut config: Config) {
        config.artists.album_display_mode = AlbumDisplayMode::NameOnly;
        config.artists.album_sort_by = AlbumSortMode::Date;
        app_context.config = std::sync::Arc::new(config);
        let mut pane = ArtistsPane::new(ArtistsPaneMode::Artist, &app_context);
        let artist = String::from("artist");
        let songs = vec![
            song("album_a", "2020"),
            song("album_b", "2019"),
            song("album_a", "2021"),
            song("album_b", "2025"),
        ];

        let CachedArtist(result) = pane.process_songs(artist, songs, &app_context);
        dbg!(&result);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "album_b");
        assert_eq!(result[1].name, "album_a");
    }
}
