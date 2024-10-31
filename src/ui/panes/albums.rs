use crate::{
    config::Config,
    context::AppContext,
    mpd::{
        commands::Song as MpdSong,
        errors::MpdError,
        mpd_client::{Filter, MpdClient, Tag},
    },
    shared::{
        key_event::KeyEvent,
        macros::{status_info, status_warn},
        mouse_event::MouseEvent,
    },
    ui::{
        browser::BrowserPane,
        dirstack::{DirStack, DirStackItem},
        widgets::browser::Browser,
        UiEvent,
    },
};

use super::{browser::DirOrSong, Pane};
use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use ratatui::{
    prelude::Rect,
    widgets::{ListItem, StatefulWidget},
    Frame,
};

#[derive(Debug)]
pub struct AlbumsPane {
    stack: DirStack<DirOrSong>,
    filter_input_mode: bool,
    browser: Browser<DirOrSong>,
}

impl AlbumsPane {
    pub fn new(context: &AppContext) -> Self {
        Self {
            stack: DirStack::default(),
            filter_input_mode: false,
            browser: Browser::new(context.config),
        }
    }
}

impl Pane for AlbumsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, _context: &AppContext) -> Result<()> {
        self.browser
            .set_filter_input_active(self.filter_input_mode)
            .render(area, frame.buffer_mut(), &mut self.stack);

        Ok(())
    }

    fn before_show(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        if self.stack().path().is_empty() {
            let result = client.list_tag(Tag::Album, None).context("Cannot list tags")?;
            self.stack = DirStack::new(
                result
                    .into_iter()
                    .map(|v| DirOrSong::Dir {
                        full_path: String::new(),
                        name: v,
                    })
                    .collect::<Vec<_>>(),
            );
            let preview = self
                .prepare_preview(client, context.config)
                .context("Cannot prepare preview")?;
            self.stack.set_preview(preview);
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        match event {
            crate::ui::UiEvent::Database => {
                let result = client.list_tag(Tag::Album, None).context("Cannot list tags")?;
                self.stack = DirStack::new(
                    result
                        .into_iter()
                        .map(|v| DirOrSong::Dir {
                            full_path: String::new(),
                            name: v,
                        })
                        .collect::<Vec<_>>(),
                );
                let preview = self
                    .prepare_preview(client, context.config)
                    .context("Cannot prepare preview")?;
                self.stack.set_preview(preview);

                status_warn!("The music database has been updated. The current tab has been reinitialized in the root directory to prevent inconsistent behaviours.");
            }
            _ => {}
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

fn list_titles(client: &mut impl MpdClient, album: &str) -> Result<impl Iterator<Item = DirOrSong>, MpdError> {
    Ok(client
        .find(&[Filter::new(Tag::Album, album)])?
        .into_iter()
        .map(DirOrSong::Song)
        .sorted())
}

fn find_songs(client: &mut impl MpdClient, album: &str, file: &str) -> Result<Vec<MpdSong>, MpdError> {
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

    fn list_songs_in_item(&self, client: &mut impl MpdClient, item: &DirOrSong) -> Result<Vec<MpdSong>> {
        match item {
            DirOrSong::Dir { name, full_path: _ } => Ok(client.find(&[Filter::new(Tag::Album, name)])?),
            DirOrSong::Song(song) => Ok(vec![song.clone()]),
        }
    }

    fn next(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        let Some(current) = self.stack.current().selected() else {
            log::error!("Failed to move deeper inside dir. Current value is None");
            return Ok(());
        };

        match self.stack.path() {
            [_album] => {
                self.add(current, client, context)?;
            }
            [] => {
                let res = list_titles(client, current.as_path())?;
                self.stack.push(res.collect());
                context.render()?;
            }
            _ => {
                log::error!("Unexpected nesting in Artists dir structure");
                context.render()?;
            }
        };

        Ok(())
    }

    fn add(&self, item: &DirOrSong, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        match self.stack.path() {
            [album] => {
                client.find_add(&[
                    Filter::new(Tag::File, &item.dir_name_or_file_name()),
                    Filter::new(Tag::Album, album.as_str()),
                ])?;

                status_info!("'{}' added to queue", item.dir_name_or_file_name());
                context.render()?;
            }
            [] => {
                client.find_add(&[Filter::new(Tag::Album, &item.dir_name_or_file_name())])?;

                status_info!("Album '{}' added to queue", &item.dir_name_or_file_name());
                context.render()?;
            }
            _ => {}
        };

        Ok(())
    }

    fn add_all(&self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        match self.stack.path() {
            [album] => {
                client.find_add(&[Filter::new(Tag::Album, album.as_str())])?;
                status_info!("Album '{}' added to queue", album);

                context.render()?;
            }
            [] => {
                client.add("/")?; // add the whole library
                status_info!("All albums added to queue");

                context.render()?;
            }
            _ => {}
        };
        Ok(())
    }

    fn prepare_preview(
        &mut self,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<Option<Vec<ListItem<'static>>>> {
        self.stack()
            .current()
            .selected()
            .map(DirStackItem::as_path)
            .map_or(Ok(None), |current| -> Result<_> {
                Ok(match self.stack.path() {
                    [album] => Some(
                        find_songs(client, album, current)?
                            .first()
                            .context(anyhow!(
                                "Expected to find exactly one song: album: '{}', current: '{}'",
                                album,
                                current
                            ))?
                            .to_preview(&config.theme.symbols)
                            .collect_vec(),
                    ),
                    [] => Some(
                        list_titles(client, current)?
                            .map(|v| v.to_list_item_simple(config))
                            .collect_vec(),
                    ),
                    _ => None,
                })
            })
    }

    fn browser_areas(&self) -> [Rect; 3] {
        self.browser.areas
    }
}
