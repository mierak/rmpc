use anyhow::{Context, Result};
use enum_map::EnumMap;
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect, widgets::ListState};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::{keys::actions::Position, tabs::PaneType},
    ctx::Ctx,
    mpd::{
        client::Client,
        commands::Song,
        mpd_client::{Filter, FilterKind, MpdClient, Tag},
    },
    shared::{
        cmp::StringCompare,
        key_event::KeyEvent,
        mouse_event::MouseEvent,
        mpd_client_ext::{Autoplay, Enqueue, MpdClientExt},
    },
    ui::{
        UiEvent,
        browser::BrowserPane,
        dir_or_song::DirOrSong,
        dirstack::{DirStack, DirStackItem},
        widgets::browser::{Browser, BrowserArea},
    },
};

#[derive(Debug)]
pub struct AlbumsPane {
    stack: DirStack<DirOrSong, ListState>,
    filter_input_mode: bool,
    browser: Browser<DirOrSong>,
    initialized: bool,
}

const INIT: &str = "init";
const FETCH_DATA: &str = "fetch_data";

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
        match &self.stack.path()[..] {
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
                self.stack_mut().enter();
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
            ctx,
        );

        Ok(())
    }

    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        if !self.initialized {
            ctx.query().id(INIT).replace_id(INIT).target(PaneType::Albums).query(move |client| {
                let result = client.list_tag(Tag::Album, None).context("Cannot list tags")?;
                Ok(MpdQueryResult::LsInfo { data: result.0, path: None })
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
                        Ok(MpdQueryResult::LsInfo { data: result.0, path: None })
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
            (FETCH_DATA, MpdQueryResult::DirOrSong { data, path }) => {
                let Some(path) = path else {
                    log::error!(path:?, current_path:? = self.stack().path(); "Cannot insert data because path is not provided");
                    return Ok(());
                };

                self.stack_mut().insert(path, data);
                self.fetch_data_internal(ctx)?;
                ctx.render()?;
            }
            (INIT, MpdQueryResult::LsInfo { data, path: _ }) => {
                let root = data
                    .into_iter()
                    .sorted_by(|a, b| {
                        StringCompare::from(ctx.config.browser_song_sort.as_ref()).compare(a, b)
                    })
                    .map(DirOrSong::name_only)
                    .collect_vec();
                self.stack = DirStack::new(root);
                self.fetch_data_internal(ctx)?;
                ctx.render()?;
            }
            _ => {}
        }
        Ok(())
    }
}

impl BrowserPane<DirOrSong> for AlbumsPane {
    fn stack(&self) -> &DirStack<DirOrSong, ListState> {
        &self.stack
    }

    fn stack_mut(&mut self) -> &mut DirStack<DirOrSong, ListState> {
        &mut self.stack
    }

    fn browser_areas(&self) -> EnumMap<BrowserArea, Rect> {
        self.browser.areas
    }

    fn set_filter_input_mode_active(&mut self, active: bool) {
        self.filter_input_mode = active;
    }

    fn is_filter_input_mode_active(&self) -> bool {
        self.filter_input_mode
    }

    fn next(&mut self, ctx: &Ctx) -> Result<()> {
        self.open_or_play(false, ctx)
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

    fn fetch_data(&self, selected: &DirOrSong, ctx: &Ctx) -> Result<()> {
        let current = selected.as_path().to_owned();
        let Some(path) = self.stack().next_path() else {
            log::error!(stack:? = self.stack; "Cannot fetch data because next path is not available");
            return Ok(());
        };

        match &self.stack.path()[..] {
            [_album] => {}
            [] => {
                let sort_order = ctx.config.browser_song_sort.clone();
                ctx.query()
                    .id(FETCH_DATA)
                    .replace_id("albums_data")
                    .target(PaneType::Albums)
                    .query(move |client| {
                        let data = client
                            .find(&[Filter::new(Tag::Album, current)])?
                            .into_iter()
                            .sorted_by(|a, b| {
                                a.with_custom_sort(&sort_order)
                                    .cmp(&b.with_custom_sort(&sort_order))
                            })
                            .map(DirOrSong::Song)
                            .collect();
                        Ok(MpdQueryResult::DirOrSong { data, path: Some(path) })
                    });
            }

            _ => {}
        }

        Ok(())
    }

    fn enqueue<'a>(
        &self,
        items: impl Iterator<Item = &'a DirOrSong>,
    ) -> (Vec<Enqueue>, Option<usize>) {
        match &self.stack.path()[..] {
            [album] => {
                let hovered = self.stack.current().selected().map(|item| item.dir_name_or_file());
                items.enumerate().fold((Vec::new(), None), |mut acc, (idx, item)| {
                    let filename = item.dir_name_or_file().into_owned();
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
                    .map(|item| item.dir_name_or_file().into_owned())
                    .map(|name| Enqueue::Find {
                        filter: vec![(Tag::Album, FilterKind::Exact, name)],
                    })
                    .collect_vec(),
                None,
            ),
            _ => (Vec::new(), None),
        }
    }

    fn open(&mut self, ctx: &Ctx) -> Result<()> {
        self.open_or_play(true, ctx)
    }

    fn initial_playlist_name(&self) -> Option<String> {
        self.stack().current().selected().and_then(|item| match item {
            DirOrSong::Dir { name, .. } => Some(name.to_owned()),
            DirOrSong::Song(_) => None,
        })
    }
}
