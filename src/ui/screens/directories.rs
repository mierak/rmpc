use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::Rect,
    style::{Color, Style, Stylize},
    widgets::{List, ListItem, ListState},
};

use crate::{
    mpd::{
        client::Client,
        commands::{list_files::ListingType, ListFiles, Listed},
        errors::MpdError,
    },
    state::State,
    ui::{Render, SharedUiState},
};

use super::Screen;

#[derive(Default, Debug)]
pub struct DirectoriesScreen {
    current_path: PathBuf,
    dirs: ListFiles,
    state: ListState,
}

#[async_trait]
impl Screen for DirectoriesScreen {
    fn render(
        &mut self,
        frame: &mut ratatui::Frame<ratatui::prelude::CrosstermBackend<std::io::Stdout>>,
        area: Rect,
        _app: &crate::state::State,
        _state: &SharedUiState,
    ) -> anyhow::Result<()> {
        let items = self
            .dirs
            .value()
            .iter()
            .map(|val| ListItem::new(format!("{:?} {}", val.kind, val.name)))
            .collect::<Vec<ListItem>>();

        let list = List::new(items)
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold())
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, area, &mut self.state);

        Ok(())
    }

    async fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        self.current_path.clear();
        self.dirs = _client.list_files(None).await.unwrap();
        if !self.dirs.value().is_empty() {
            self.state.select(Some(0));
            self.dirs.value_mut().sort_by(|a, b| a.name.cmp(&b.name));
        };

        Ok(())
    }

    async fn handle_key(
        &mut self,
        key: KeyEvent,
        _client: &mut Client<'_>,
        _app: &mut State,
    ) -> Result<Render, MpdError> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let i = match self.state.selected() {
                    Some(i) => {
                        if i >= self.dirs.value().len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.state.select(Some(i));
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let i = match self.state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.dirs.value().len() - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.state.select(Some(i));
                return Ok(Render::NoSkip);
            }
            KeyCode::Enter | KeyCode::Char('l') => {
                if let Some(i) = self.state.selected() {
                    let selected = &self.dirs.value()[i];
                    match selected.kind {
                        ListingType::Dir => {
                            if selected.name == ".." {
                                self.current_path.pop();
                            } else {
                                self.current_path.push(&selected.name);
                            }
                            self.dirs = _client
                                .list_files(Some(self.current_path.to_str().unwrap()))
                                .await
                                .unwrap();
                            if self.current_path.to_string_lossy() != "" {
                                self.dirs.value_mut().push(Listed {
                                    kind: ListingType::Dir,
                                    name: "..".to_owned(),
                                    size: 0,
                                    last_modified: "".to_owned(),
                                });
                            }
                            self.dirs.value_mut().sort_by(|a, b| a.name.cmp(&b.name));
                        }
                        ListingType::File => {
                            let mut song_path = self.current_path.clone();
                            song_path.push(&selected.name);
                            _client.add(song_path.to_str().unwrap()).await.unwrap();
                        }
                    }
                }
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('h') => {
                self.current_path.pop();
                self.dirs = _client
                    .list_files(Some(self.current_path.to_str().unwrap()))
                    .await
                    .unwrap();
                if self.current_path.to_string_lossy() != "" {
                    self.dirs.value_mut().push(Listed {
                        kind: ListingType::Dir,
                        name: "..".to_owned(),
                        size: 0,
                        last_modified: "".to_owned(),
                    });
                }
                self.dirs.value_mut().sort_by(|a, b| a.name.cmp(&b.name));
                return Ok(Render::NoSkip);
            }
            _ => {}
        }
        Ok(Render::Skip)
    }
}
