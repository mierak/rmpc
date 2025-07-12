use anyhow::{Context, Result, anyhow};
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::{keys::actions::Position, sort_mode::SortOptions, tabs::PaneType},
    ctx::Ctx,
    mpd::{
        client::Client,
        commands::Song,
        errors::MpdError,
        mpd_client::{Filter, FilterKind, MpdClient, Tag},
    },
    shared::{
        key_event::KeyEvent,
        mouse_event::MouseEvent,
        mpd_client_ext::{Autoplay, Enqueue, MpdClientExt},
        mpd_query::PreviewGroup,
    },
    ui::{
        UiEvent,
        browser::BrowserPane,
        dir_or_song::DirOrSong,
        dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
    },
};

#[derive(Debug)]
pub struct AlbumsPane {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
    browser: Browser<DirOrSong>,
    initialized: bool,
}

const INIT: &str = "init";
const OPEN_OR_PLAY: &str = "open_or_play";
const PREVIEW: &str = "preview";

impl AlbumsPane {
    pub fn new(_ctx: &Ctx) -> Self {
        Self {
            stack: DirStack::default(),
            filter_input_mode: false,
            browser: Browser::new(),
            initialized: false,
        }
    }

    fn open_or_play(&mut self, autoplay: bool, ctx: &Ctx) -> Result<()> {
        let Some(current) = self.stack.current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };
        let Some(next_path) = self.stack.next_path() else {
            log::error!("Failed to move deeper inside dir. Next path is None");
            return Ok(());
        };

        match self.stack.path() {
            [_album] => {
                let (items, hovered_song_idx) = self.enqueue(self.stack().current().items.iter());
                let queue_len = ctx.queue.len();
                let (position, autoplay) = if autoplay {
                    (Position::Replace, Autoplay::Hovered {
                        queue_len,
                        current_song_idx: None,
                        hovered_song_idx,
                    })
                } else {
                    (Position::EndOfQueue, Autoplay::None)
                };
                if !items.is_empty() {
                    ctx.command(move |client| {
                        client.enqueue_multiple(items, position, autoplay)?;
                        Ok(())
                    });
                }
            }
            [] => {
                let current = current.clone();
                let sort_order = ctx.config.browser_song_sort.clone();
                ctx.query()
                    .id(OPEN_OR_PLAY)
                    .replace_id(OPEN_OR_PLAY)
                    .target(PaneType::Albums)
                    .query(move |client| {
                        let data = list_titles(client, current.as_path(), &sort_order)?.collect();
                        Ok(MpdQueryResult::DirOrSong { data, origin_path: Some(next_path) })
                    });
                self.stack_mut().push(Vec::new());
                self.stack_mut().clear_preview();
                ctx.render()?;
            }
            _ => {
                log::error!("Unexpected nesting in Artists dir structure");
                ctx.render()?;
            }
        }

        Ok(())
    }
}

impl Pane for AlbumsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        self.browser.set_filter_input_active(self.filter_input_mode).render(
            area,
            frame.buffer_mut(),
            &mut self.stack,
            &ctx.config,
        );

        Ok(())
    }

    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        if !self.initialized {
            ctx.query().id(INIT).replace_id(INIT).target(PaneType::Albums).query(move |client| {
                let result = client.list_tag(Tag::Album, None).context("Cannot list tags")?;
                Ok(MpdQueryResult::LsInfo { data: result.0, origin_path: None })
            });
            self.initialized = true;
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, _is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::Database => {
                ctx.query().id(INIT).replace_id(INIT).target(PaneType::Albums).query(
                    move |client| {
                        let result =
                            client.list_tag(Tag::Album, None).context("Cannot list tags")?;
                        Ok(MpdQueryResult::LsInfo { data: result.0, origin_path: None })
                    },
                );
            }
            UiEvent::Reconnected => {
                self.initialized = false;
                self.before_show(ctx)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        self.handle_mouse_action(event, ctx)
    }

    fn handle_action(&mut self, event: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        self.handle_filter_input(event, ctx)?;
        self.handle_common_action(event, ctx)?;
        self.handle_global_action(event, ctx)?;
        Ok(())
    }

    fn on_query_finished(
        &mut self,
        id: &'static str,
        data: MpdQueryResult,
        _is_visible: bool,
        ctx: &Ctx,
    ) -> Result<()> {
        match (id, data) {
            (PREVIEW, MpdQueryResult::Preview { data, origin_path }) => {
                if let Some(origin_path) = origin_path {
                    if origin_path != self.stack().path() {
                        log::trace!(origin_path:?, current_path:? = self.stack().path(); "Dropping preview because it does not belong to this path");
                        return Ok(());
                    }
                }
                self.stack_mut().set_preview(data);
                ctx.render()?;
            }
            (INIT, MpdQueryResult::LsInfo { data, origin_path: _ }) => {
                let root = data.into_iter().map(DirOrSong::name_only).collect_vec();
                self.stack = DirStack::new(root);
                self.prepare_preview(ctx)?;
            }
            (OPEN_OR_PLAY, MpdQueryResult::DirOrSong { data, origin_path }) => {
                if let Some(origin_path) = origin_path {
                    if origin_path != self.stack().path() {
                        log::trace!(origin_path:?, current_path:? = self.stack().path(); "Dropping result because it does not belong to this path");
                        return Ok(());
                    }
                }
                self.stack_mut().replace(data);
                self.prepare_preview(ctx)?;
                ctx.render()?;
            }
            _ => {}
        }
        Ok(())
    }
}

fn list_titles(
    client: &mut impl MpdClient,
    album: &str,
    sort_opts: &SortOptions,
) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .find(&[Filter::new(Tag::Album, album)])?
        .into_iter()
        .sorted_by(|a, b| a.with_custom_sort(sort_opts).cmp(&b.with_custom_sort(sort_opts)))
        .map(DirOrSong::Song))
}

fn find_songs(
    client: &mut impl MpdClient,
    album: &str,
    file: &str,
    sort_opts: &SortOptions,
) -> Result<Song, MpdError> {
    Ok(client
        .find(&[Filter::new(Tag::File, file), Filter::new(Tag::Album, album)])?
        .into_iter()
        .sorted_by(|a, b| a.with_custom_sort(sort_opts).cmp(&b.with_custom_sort(sort_opts)))
        .next()
        .context(anyhow!(
            "Expected to find exactly one song: album: '{}', current: '{}'",
            album,
            file
        ))?)
}

impl BrowserPane<DirOrSong> for AlbumsPane {
    fn stack(&self) -> &DirStack<DirOrSong> {
        &self.stack
    }

    fn stack_mut(&mut self) -> &mut DirStack<DirOrSong> {
        &mut self.stack
    }

    fn initial_playlist_name(&self) -> Option<String> {
        self.stack().current().selected().and_then(|item| match item {
            DirOrSong::Dir { name, .. } => Some(name.to_owned()),
            DirOrSong::Song(_) => None,
        })
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
    ) -> impl FnOnce(&mut Client<'_>) -> Result<Vec<Song>> + Clone + 'static {
        move |client| match item {
            DirOrSong::Dir { name, .. } => Ok(client.find(&[Filter::new(Tag::Album, &name)])?),
            DirOrSong::Song(song) => Ok(vec![song.clone()]),
        }
    }

    fn open(&mut self, ctx: &Ctx) -> Result<()> {
        self.open_or_play(true, ctx)
    }

    fn next(&mut self, ctx: &Ctx) -> Result<()> {
        self.open_or_play(false, ctx)
    }

    fn enqueue<'a>(
        &self,
        items: impl Iterator<Item = &'a DirOrSong>,
    ) -> (Vec<Enqueue>, Option<usize>) {
        match self.stack.path() {
            [album] => {
                let hovered =
                    self.stack.current().selected().map(|item| item.dir_name_or_file_name());
                items.enumerate().fold((Vec::new(), None), |mut acc, (idx, item)| {
                    let filename = item.dir_name_or_file_name().into_owned();
                    if hovered.as_ref().is_some_and(|hovered| hovered == &filename) {
                        acc.1 = Some(idx);
                    }
                    acc.0.push(Enqueue::Find {
                        filter: vec![
                            (Tag::File, FilterKind::Exact, filename),
                            (Tag::Album, FilterKind::Exact, album.clone()),
                        ],
                    });

                    acc
                })
            }
            [] => (
                items
                    .map(|item| item.dir_name_or_file_name().into_owned())
                    .map(|name| Enqueue::Find {
                        filter: vec![(Tag::Album, FilterKind::Exact, name)],
                    })
                    .collect_vec(),
                None,
            ),
            _ => (Vec::new(), None),
        }
    }

    fn prepare_preview(&mut self, ctx: &Ctx) -> Result<()> {
        let Some(current) = self.stack().current().selected().map(DirStackItem::as_path) else {
            return Ok(());
        };
        let current = current.to_owned();
        let config = std::sync::Arc::clone(&ctx.config);
        let origin_path = Some(self.stack().path().to_vec());

        self.stack_mut().clear_preview();
        match self.stack.path() {
            [album] => {
                let album = album.clone();
                let sort_order = ctx.config.browser_song_sort.clone();
                ctx.query()
                    .id(PREVIEW)
                    .replace_id("albums_preview")
                    .target(PaneType::Albums)
                    .query(move |client| {
                        let data =
                            Some(find_songs(client, &album, &current, &sort_order)?.to_preview(
                                config.theme.preview_label_style,
                                config.theme.preview_metadata_group_style,
                            ));
                        Ok(MpdQueryResult::Preview { data, origin_path })
                    });
            }
            [] => {
                let sort_order = ctx.config.browser_song_sort.clone();
                ctx.query()
                    .id(PREVIEW)
                    .replace_id("albums_preview")
                    .target(PaneType::Albums)
                    .query(move |client| {
                        let data = list_titles(client, &current, &sort_order)?
                            .map(|v| v.to_list_item_simple(&config))
                            .collect_vec();
                        let data = PreviewGroup::from(None, None, data);
                        let data = Some(vec![data]);
                        Ok(MpdQueryResult::Preview { data, origin_path })
                    });
            }

            _ => {}
        }

        Ok(())
    }

    fn browser_areas(&self) -> [Rect; 3] {
        self.browser.areas
    }
}
