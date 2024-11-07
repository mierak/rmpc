use anyhow::Result;
use itertools::Itertools;
use ratatui::{
    prelude::Rect,
    widgets::{ListItem, StatefulWidget},
    Frame,
};

use crate::{
    config::Config,
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

    fn open_or_play(&mut self, autoplay: bool, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
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
                let new_current = client.lsinfo(Some(next_path.join("/").to_string().as_str()))?;
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
                self.stack.push(res);

                context.render()?;
            }
            t @ DirOrSong::Song(_) => {
                self.add(t, client, context)?;
                if autoplay {
                    client.play_last(context)?;
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

    fn before_show(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        if !self.initialized {
            self.stack = DirStack::new(
                client
                    .lsinfo(None)?
                    .into_iter()
                    .map(Into::<DirOrSong>::into)
                    .sorted()
                    .collect::<Vec<_>>(),
            );
            let preview = self.prepare_preview(client, context.config)?;
            self.stack.set_preview(preview);
            self.initialized = true;
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        if let crate::ui::UiEvent::Database = event {
            self.stack = DirStack::new(
                client
                    .lsinfo(None)?
                    .into_iter()
                    .map(Into::<DirOrSong>::into)
                    .collect::<Vec<_>>(),
            );
            let preview = self.prepare_preview(client, context.config)?;
            self.stack.set_preview(preview);

            context.render()?;
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

    fn list_songs_in_item(&self, client: &mut impl MpdClient, item: &DirOrSong) -> Result<Vec<Song>> {
        Ok(match item {
            DirOrSong::Dir { full_path, .. } => {
                client.find(&[Filter::new_with_kind(Tag::File, full_path, FilterKind::StartsWith)])?
            }
            DirOrSong::Song(song) => vec![song.clone()],
        })
    }

    fn add(&self, item: &DirOrSong, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        match item {
            DirOrSong::Dir {
                name: dirname,
                full_path: _,
            } => {
                let mut next_path = self.stack.path().to_vec();
                next_path.push(dirname.clone());
                let next_path = next_path.join(std::path::MAIN_SEPARATOR_STR).to_string();

                client.add(&next_path)?;
                status_info!("Directory '{next_path}' added to queue");
            }
            DirOrSong::Song(song) => {
                client.add(&song.file)?;
                if let Ok(Some(song)) = client.find_one(&[Filter::new(Tag::File, &song.file)]) {
                    status_info!("'{}' by '{}' added to queue", song.title_str(), song.artist_str());
                }
            }
        };

        context.render()?;

        Ok(())
    }

    fn add_all(&self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        let path = self.stack().path().join(std::path::MAIN_SEPARATOR_STR);
        client.add(&path)?;
        status_info!("Directory '{path}' added to queue");

        context.render()?;

        Ok(())
    }

    fn open(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        self.open_or_play(true, client, context)
    }

    fn next(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        self.open_or_play(false, client, context)
    }

    fn prepare_preview(
        &mut self,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<Option<Vec<ListItem<'static>>>> {
        match &self.stack.current().selected() {
            Some(DirOrSong::Dir { .. }) => {
                let Some(next_path) = self.stack.next_path() else {
                    log::error!("Failed to move deeper inside dir. Next path is None");
                    return Ok(None);
                };
                let res: Vec<_> = match client.lsinfo(Some(&next_path.join("/").to_string())) {
                    Ok(val) => val,
                    Err(err) => {
                        log::error!(error:? = err; "Failed to get lsinfo for dir",);
                        return Ok(None);
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
                Ok(Some(res))
            }
            Some(DirOrSong::Song(song)) => Ok(client
                .find_one(&[Filter::new(Tag::File, &song.file)])?
                .map(|v| v.to_preview(&config.theme.symbols).collect())),
            None => Ok(None),
        }
    }
    fn browser_areas(&self) -> [Rect; 3] {
        self.browser.areas
    }
}
