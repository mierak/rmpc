use anyhow::Result;
use itertools::Itertools;
use ratatui::{Frame, prelude::Rect, widgets::StatefulWidget};

use super::{Pane, browser::DirOrSong};
use crate::{
    MpdQueryResult,
    config::tabs::PaneType,
    context::AppContext,
    mpd::{
        client::Client,
        commands::{Song, lsinfo::LsInfoEntry},
        mpd_client::{Filter, FilterKind, MpdClient, Tag},
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
pub struct DirectoriesPane {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
    browser: Browser<DirOrSong>,
    initialized: bool,
}

const INIT: &str = "init";
const OPEN_OR_PLAY: &str = "open_or_play";
const PREVIEW: &str = "preview";

impl DirectoriesPane {
    pub fn new(context: &AppContext) -> Self {
        Self {
            stack: DirStack::default(),
            filter_input_mode: false,
            browser: Browser::new(context.config),
            initialized: false,
        }
    }

    fn open_or_play(&mut self, autoplay: bool, context: &AppContext) -> Result<()> {
        let Some(selected) = self.stack.current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };
        let Some(next_path) = self.stack.next_path() else {
            log::error!("Failed to move deeper inside dir. Next path is None");
            return Ok(());
        };

        match selected {
            DirOrSong::Dir { .. } => {
                context
                    .query()
                    .id(OPEN_OR_PLAY)
                    .replace_id(OPEN_OR_PLAY)
                    .target(PaneType::Directories)
                    .query(move |client| {
                        let new_current = client.lsinfo(Some(&next_path.join("/").to_string()))?;
                        let res = new_current
                            .into_iter()
                            .filter_map(|v| match v {
                                LsInfoEntry::Dir(d) => {
                                    Some(DirOrSong::Dir { name: d.path, full_path: d.full_path })
                                }
                                LsInfoEntry::File(s) => Some(DirOrSong::Song(s)),
                                LsInfoEntry::Playlist(_) => None,
                            })
                            .sorted()
                            .collect();

                        Ok(MpdQueryResult::DirOrSong { data: res, origin_path: Some(next_path) })
                    });
                self.stack_mut().push(Vec::new());
                self.stack_mut().clear_preview();
                context.render()?;
            }
            t @ DirOrSong::Song(_) => {
                self.add(t, context)?;
                let queue_len = context.queue.len();
                if autoplay {
                    context.command(move |client| Ok(client.play_last(queue_len)?));
                }
            }
        };

        Ok(())
    }
}

impl Pane for DirectoriesPane {
    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        _context: &AppContext,
    ) -> anyhow::Result<()> {
        self.browser.set_filter_input_active(self.filter_input_mode).render(
            area,
            frame.buffer_mut(),
            &mut self.stack,
        );

        Ok(())
    }

    fn before_show(&mut self, context: &AppContext) -> Result<()> {
        if !self.initialized {
            context.query().id(INIT).replace_id(INIT).target(PaneType::Directories).query(
                move |client| {
                    let result = client
                        .lsinfo(None)?
                        .into_iter()
                        .filter_map(Into::<Option<DirOrSong>>::into)
                        .sorted()
                        .collect::<Vec<_>>();
                    Ok(MpdQueryResult::DirOrSong { data: result, origin_path: None })
                },
            );
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
                context.query().id(INIT).replace_id(INIT).target(PaneType::Directories).query(
                    move |client| {
                        let result = client
                            .lsinfo(None)?
                            .into_iter()
                            .filter_map(Into::<Option<DirOrSong>>::into)
                            .sorted()
                            .collect::<Vec<_>>();
                        Ok(MpdQueryResult::DirOrSong { data: result, origin_path: None })
                    },
                );
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
            (INIT, MpdQueryResult::DirOrSong { data, origin_path: _ }) => {
                self.stack = DirStack::new(data);
                self.prepare_preview(context)?;
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
                self.prepare_preview(context)?;
                context.render()?;
            }
            _ => {}
        };
        Ok(())
    }
}

impl BrowserPane<DirOrSong> for DirectoriesPane {
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
        move |client| {
            Ok(match item {
                DirOrSong::Dir { full_path, .. } => client.find(&[Filter::new_with_kind(
                    Tag::File,
                    &full_path,
                    FilterKind::StartsWith,
                )])?,
                DirOrSong::Song(song) => vec![song.clone()],
            })
        }
    }

    fn add(&self, item: &DirOrSong, context: &AppContext) -> Result<()> {
        match item {
            DirOrSong::Dir { name: dirname, full_path: _ } => {
                let mut next_path = self.stack.path().to_vec();
                next_path.push(dirname.clone());
                let next_path = next_path.join(std::path::MAIN_SEPARATOR_STR).to_string();

                context.command(move |client| {
                    client.add(&next_path)?;
                    status_info!("Directory '{next_path}' added to queue");
                    Ok(())
                });
            }
            DirOrSong::Song(song) => {
                let file = song.file.clone();
                context.command(move |client| {
                    client.add(&file)?;
                    if let Ok(Some(song)) = client.find_one(&[Filter::new(Tag::File, &file)]) {
                        status_info!(
                            "'{}' by '{}' added to queue",
                            song.title_str(),
                            song.artist_str()
                        );
                    }
                    Ok(())
                });
            }
        };

        context.render()?;

        Ok(())
    }

    fn add_all(&self, context: &AppContext) -> Result<()> {
        let path = self.stack().path().join(std::path::MAIN_SEPARATOR_STR);
        context.command(move |client| {
            client.add(&path)?;
            status_info!("Directory '{path}' added to queue");
            Ok(())
        });

        Ok(())
    }

    fn open(&mut self, context: &AppContext) -> Result<()> {
        self.open_or_play(true, context)
    }

    fn next(&mut self, context: &AppContext) -> Result<()> {
        self.open_or_play(false, context)
    }

    fn prepare_preview(&mut self, context: &AppContext) -> Result<()> {
        let origin_path = Some(self.stack().path().to_vec());
        match &self.stack.current().selected() {
            Some(DirOrSong::Dir { .. }) => {
                let Some(next_path) = self.stack.next_path() else {
                    log::error!("Failed to move deeper inside dir. Next path is None");
                    return Ok(());
                };
                let next_path = next_path.join("/").to_string();
                let config = context.config;

                self.stack_mut().clear_preview();
                context
                    .query()
                    .id(PREVIEW)
                    .replace_id("directories_preview")
                    .target(PaneType::Directories)
                    .query(move |client| {
                        let data: Vec<_> = match client.lsinfo(Some(&next_path)) {
                            Ok(val) => val,
                            Err(err) => {
                                log::error!(error:? = err; "Failed to get lsinfo for dir",);
                                return Ok(MpdQueryResult::Preview {
                                    data: None,
                                    origin_path: None,
                                });
                            }
                        }
                        .0
                        .into_iter()
                        .filter_map(|v| match v {
                            LsInfoEntry::Dir(dir) => {
                                Some(DirOrSong::Dir { name: dir.path, full_path: dir.full_path })
                            }
                            LsInfoEntry::File(song) => Some(DirOrSong::Song(song)),
                            LsInfoEntry::Playlist(_) => None,
                        })
                        .sorted()
                        .map(|v| v.to_list_item_simple(config))
                        .collect();

                        Ok(MpdQueryResult::Preview {
                            data: Some(vec![PreviewGroup::from(None, data)]),
                            origin_path,
                        })
                    });
            }
            Some(DirOrSong::Song(song)) => {
                let file = song.file.clone();
                let config = context.config;
                context
                    .query()
                    .id(PREVIEW)
                    .replace_id("directories_preview")
                    .target(PaneType::Directories)
                    .query(move |client| {
                        Ok(MpdQueryResult::Preview {
                            data: client
                                .find_one(&[Filter::new(Tag::File, &file)])?
                                .map(|v| v.to_preview(&config.theme.symbols)),
                            origin_path,
                        })
                    });
            }
            None => {}
        };
        Ok(())
    }

    fn browser_areas(&self) -> [Rect; 3] {
        self.browser.areas
    }
}
