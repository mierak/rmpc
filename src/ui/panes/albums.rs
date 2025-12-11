use anyhow::{Context, Result};
use enum_map::EnumMap;
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect, widgets::ListState};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::tabs::PaneType,
    ctx::Ctx,
    mpd::{
        client::Client,
        commands::Song,
        mpd_client::{Filter, MpdClient, Tag},
    },
    shared::{cmp::StringCompare, key_event::KeyEvent, mouse_event::MouseEvent},
    ui::{
        UiEvent,
        browser::BrowserPane,
        dir_or_song::DirOrSong,
        dirstack::{DirStack, DirStackItem},
        input::InputResultEvent,
        widgets::browser::{Browser, BrowserArea},
    },
};

#[derive(Debug)]
pub struct AlbumsPane {
    stack: DirStack<DirOrSong, ListState>,
    browser: Browser<DirOrSong>,
    initialized: bool,
}

const INIT: &str = "init";
const FETCH_DATA: &str = "fetch_data";

impl AlbumsPane {
    pub fn new(_ctx: &Ctx) -> Self {
        Self { stack: DirStack::default(), browser: Browser::new(), initialized: false }
    }
}

impl Pane for AlbumsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        self.browser.render(area, frame.buffer_mut(), &mut self.stack, ctx);

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

    fn handle_insert_mode(&mut self, kind: InputResultEvent, ctx: &mut Ctx) -> Result<()> {
        BrowserPane::handle_insert_mode(self, kind, ctx)?;
        Ok(())
    }

    fn handle_action(&mut self, event: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
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

        match self.stack.path().as_slice() {
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
}
