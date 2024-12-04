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
    pub fn new(context: &AppContext) -> Self {
        Self {
            stack: DirStack::default(),
            filter_input_mode: false,
            browser: Browser::new(context.config),
            initialized: false,
        }
    }

    fn open_or_play(&mut self, autoplay: bool, context: &AppContext) -> Result<()> {
        let Some(current) = self.stack.current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };

        match self.stack.path() {
            [_album] => {
                self.add(current, context)?;
                let queue_len = context.queue.len();
                if autoplay {
                    context.command(move |client| Ok(client.play_last(queue_len)?));
                }
            }
            [] => {
                let current = current.clone();
                context.query(OPEN_OR_PLAY, PaneType::Albums, move |client| {
                    let res = list_titles(client, current.as_path())?.collect();
                    Ok(MpdQueryResult::DirOrSong(res))
                });
                self.stack_mut().push(Vec::new());
                self.stack_mut().clear_preview();
                context.render()?;
            }
            _ => {
                log::error!("Unexpected nesting in Artists dir structure");
                context.render()?;
            }
        };

        Ok(())
    }
}

impl Pane for AlbumsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, _context: &AppContext) -> Result<()> {
        self.browser
            .set_filter_input_active(self.filter_input_mode)
            .render(area, frame.buffer_mut(), &mut self.stack);

        Ok(())
    }

    fn before_show(&mut self, context: &AppContext) -> Result<()> {
        if !self.initialized {
            context.query(INIT, PaneType::Albums, move |client| {
                let result = client.list_tag(Tag::Album, None).context("Cannot list tags")?;
                Ok(MpdQueryResult::LsInfo(result.0))
            });
            self.initialized = true;
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, context: &AppContext) -> Result<()> {
        if let crate::ui::UiEvent::Database = event {
            context.query(INIT, PaneType::Albums, move |client| {
                let result = client.list_tag(Tag::Album, None).context("Cannot list tags")?;
                Ok(MpdQueryResult::LsInfo(result.0))
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
            (PREVIEW, MpdQueryResult::Preview(vec)) => {
                self.stack_mut().set_preview(vec);
                context.render()?;
            }
            (INIT, MpdQueryResult::LsInfo(data)) => {
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
            (OPEN_OR_PLAY, MpdQueryResult::DirOrSong(data)) => {
                self.stack_mut().replace(data);
                self.prepare_preview(context);
                context.render()?;
            }
            _ => {}
        };
        Ok(())
    }
}

fn list_titles(client: &mut impl MpdClient, album: &str) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .find(&[Filter::new(Tag::Album, album)])?
        .into_iter()
        .map(DirOrSong::Song)
        .sorted())
}

fn find_songs(client: &mut impl MpdClient, album: &str, file: &str) -> Result<Vec<Song>, MpdError> {
    client
        .find(&[Filter::new(Tag::File, file), Filter::new(Tag::Album, album)])
        .map(|mut v| {
            v.sort();
            v
        })
}

impl BrowserPane<DirOrSong> for AlbumsPane {
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
        move |client| match item {
            DirOrSong::Dir { name, full_path: _ } => Ok(client.find(&[Filter::new(Tag::Album, &name)])?),
            DirOrSong::Song(song) => Ok(vec![song.clone()]),
        }
    }

    fn open(&mut self, context: &AppContext) -> Result<()> {
        self.open_or_play(true, context)
    }

    fn next(&mut self, context: &AppContext) -> Result<()> {
        self.open_or_play(false, context)
    }

    fn add(&self, item: &DirOrSong, context: &AppContext) -> Result<()> {
        match self.stack.path() {
            [album] => {
                let album = album.clone();
                let name = item.dir_name_or_file_name().into_owned();
                context.command(move |client| {
                    client.find_add(&[Filter::new(Tag::File, &name), Filter::new(Tag::Album, album.as_str())])?;

                    status_info!("'{name}' added to queue");
                    Ok(())
                });
            }
            [] => {
                let name = item.dir_name_or_file_name().into_owned();
                context.command(move |client| {
                    client.find_add(&[Filter::new(Tag::Album, &name)])?;

                    status_info!("Album '{name}' added to queue");
                    Ok(())
                });
            }
            _ => {}
        };

        Ok(())
    }

    fn add_all(&self, context: &AppContext) -> Result<()> {
        match self.stack.path() {
            [album] => {
                let album = album.clone();
                context.command(move |client| {
                    client.find_add(&[Filter::new(Tag::Album, album.as_str())])?;
                    status_info!("Album '{}' added to queue", album);
                    Ok(())
                });
            }
            [] => {
                context.command(move |client| {
                    client.add("/")?; // add the whole library
                    status_info!("All albums added to queue");
                    Ok(())
                });
            }
            _ => {}
        };
        Ok(())
    }

    fn prepare_preview(&self, context: &AppContext) {
        let Some(current) = self.stack().current().selected().map(DirStackItem::as_path) else {
            return;
        };
        let current = current.to_owned();
        let config = context.config;

        match self.stack.path() {
            [album] => {
                let album = album.clone();
                context.query_replaceable(PREVIEW, "albums_preview", PaneType::Albums, move |client| {
                    let result = Some(
                        find_songs(client, &album, &current)?
                            .first()
                            .context(anyhow!(
                                "Expected to find exactly one song: album: '{}', current: '{}'",
                                album,
                                current
                            ))?
                            .to_preview(&config.theme.symbols)
                            .collect_vec(),
                    );
                    Ok(MpdQueryResult::Preview(result))
                });
            }
            [] => {
                context.query_replaceable(PREVIEW, "albums_preview", PaneType::Albums, move |client| {
                    let result = Some(
                        list_titles(client, &current)?
                            .map(|v| v.to_list_item_simple(config))
                            .collect_vec(),
                    );
                    Ok(MpdQueryResult::Preview(result))
                });
            }

            _ => {}
        }
    }

    fn browser_areas(&self) -> [Rect; 3] {
        self.browser.areas
    }
}
