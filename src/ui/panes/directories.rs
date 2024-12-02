use anyhow::Result;
use itertools::Itertools;
use ratatui::{prelude::Rect, widgets::StatefulWidget, Frame};

use crate::{
    config::tabs::PaneType,
    context::AppContext,
    mpd::{
        commands::{lsinfo::FileOrDir, Song},
        mpd_client::{Filter, FilterKind, MpdClient, Tag},
    },
    shared::{ext::mpd_client::MpdClientExt, key_event::KeyEvent, macros::status_info, mouse_event::MouseEvent},
    ui::{
        browser::BrowserPane,
        dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        UiEvent,
    },
    MpdCommandResult,
};

use super::{browser::DirOrSong, Pane};

#[derive(Debug)]
pub struct DirectoriesPane {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
    browser: Browser<DirOrSong>,
    initialized: bool,
}

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
                let next_path = next_path.join("/").to_string();
                context.query("next", PaneType::Directories, move |client| {
                    let new_current = client.lsinfo(Some(&next_path))?;
                    let res = new_current
                        .into_iter()
                        .map(|v| match v {
                            FileOrDir::Dir(d) => DirOrSong::Dir {
                                name: d.path,
                                full_path: d.full_path,
                            },
                            FileOrDir::File(s) => DirOrSong::Song(s),
                        })
                        .sorted()
                        .collect();

                    Ok(MpdCommandResult::DirOrSong(res))
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
    fn render(&mut self, frame: &mut Frame, area: Rect, _context: &AppContext) -> anyhow::Result<()> {
        self.browser
            .set_filter_input_active(self.filter_input_mode)
            .render(area, frame.buffer_mut(), &mut self.stack);

        Ok(())
    }

    fn before_show(&mut self, context: &AppContext) -> Result<()> {
        if !self.initialized {
            context.query("init", PaneType::Directories, move |client| {
                let result = client
                    .lsinfo(None)?
                    .into_iter()
                    .map(Into::<DirOrSong>::into)
                    .sorted()
                    .collect::<Vec<_>>();
                Ok(MpdCommandResult::DirOrSong(result))
            });
            self.initialized = true;
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, context: &AppContext) -> Result<()> {
        if let crate::ui::UiEvent::Database = event {
            context.query("init", PaneType::Directories, move |client| {
                let result = client
                    .lsinfo(None)?
                    .into_iter()
                    .map(Into::<DirOrSong>::into)
                    .sorted()
                    .collect::<Vec<_>>();
                Ok(MpdCommandResult::DirOrSong(result))
            });
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
                if id == "init" {
                    self.stack = DirStack::new(data);
                } else {
                    self.stack_mut().replace(data);
                }
                self.prepare_preview(context);
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

    fn list_songs_in_item(client: &mut impl MpdClient, item: &DirOrSong) -> Result<Vec<Song>> {
        Ok(match item {
            DirOrSong::Dir { full_path, .. } => {
                client.find(&[Filter::new_with_kind(Tag::File, full_path, FilterKind::StartsWith)])?
            }
            DirOrSong::Song(song) => vec![song.clone()],
        })
    }

    fn add(&self, item: &DirOrSong, context: &AppContext) -> Result<()> {
        match item {
            DirOrSong::Dir {
                name: dirname,
                full_path: _,
            } => {
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
                        status_info!("'{}' by '{}' added to queue", song.title_str(), song.artist_str());
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

    fn prepare_preview(&self, context: &AppContext) {
        match &self.stack.current().selected() {
            Some(DirOrSong::Dir { .. }) => {
                let Some(next_path) = self.stack.next_path() else {
                    log::error!("Failed to move deeper inside dir. Next path is None");
                    return;
                };
                let next_path = next_path.join("/").to_string();
                let config = context.config;

                context.query("preview", PaneType::Directories, move |client| {
                    let res: Vec<_> = match client.lsinfo(Some(&next_path)) {
                        Ok(val) => val,
                        Err(err) => {
                            log::error!(error:? = err; "Failed to get lsinfo for dir",);
                            return Ok(MpdCommandResult::Preview(None));
                        }
                    }
                    .0
                    .into_iter()
                    .map(|v| match v {
                        FileOrDir::Dir(dir) => DirOrSong::Dir {
                            name: dir.path,
                            full_path: dir.full_path,
                        },
                        FileOrDir::File(song) => DirOrSong::Song(song),
                    })
                    .sorted()
                    .map(|v| v.to_list_item_simple(config))
                    .collect();

                    Ok(MpdCommandResult::Preview(Some(res)))
                });
            }
            Some(DirOrSong::Song(song)) => {
                let file = song.file.clone();
                let config = context.config;
                context.query("preview", PaneType::Directories, move |client| {
                    Ok(MpdCommandResult::Preview(
                        client
                            .find_one(&[Filter::new(Tag::File, &file)])?
                            .map(|v| v.to_preview(&config.theme.symbols).collect()),
                    ))
                });
            }
            None => {}
        }
    }

    fn browser_areas(&self) -> [Rect; 3] {
        self.browser.areas
    }
}
