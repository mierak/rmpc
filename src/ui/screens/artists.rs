use crate::{
    mpd::{
        client::{Client, Filter},
        commands::Song as MpdSong,
        errors::MpdError,
    },
    state::State,
    ui::{screens::directories::FileOrDirExt, Level, Render, SharedUiState, StatusMessage},
};

use super::{DirStack, Screen};
use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::Rect, widgets::ListItem, Frame};
use ratatui::{
    prelude::{Constraint, Layout},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, List, Scrollbar, ScrollbarOrientation},
};

#[derive(Debug)]
pub struct ArtistsScreen {
    stack: DirStack<String>,
    position: CurrentPosition,
    next: Vec<ListItem<'static>>,
}

impl Default for ArtistsScreen {
    fn default() -> Self {
        Self {
            stack: DirStack::new(Vec::new()),
            position: CurrentPosition::Artist(Position { values: Artist }),
            next: Vec::new(),
        }
    }
}

impl ArtistsScreen {
    async fn prepare_preview(&mut self, client: &mut Client<'_>) -> Option<Vec<ListItem<'static>>> {
        self.stack.current.1.get_selected()?;
        let current = &self.stack.current.0[self.stack.current.1.get_selected().unwrap()];
        Some(match &self.position {
            CurrentPosition::Artist(val) => val.fetch(client, current).await.unwrap().to_listitems(false),
            CurrentPosition::Album(val) => val.fetch(client, current).await.unwrap().to_listitems(true),
            CurrentPosition::Song(val) => val.fetch(client, current).await.unwrap().to_listitems(),
        })
    }
}

#[async_trait]
impl Screen for ArtistsScreen {
    fn render<B: ratatui::prelude::Backend>(
        &mut self,
        frame: &mut Frame<B>,
        area: Rect,
        _app: &mut State,
        _shared_state: &mut SharedUiState,
    ) -> Result<()> {
        let [previous_area, current_area, preview_area] = *Layout::default()
            .direction(ratatui::prelude::Direction::Horizontal)
            // cfg
            .constraints([
                         Constraint::Percentage(20),
                         Constraint::Percentage(38),
                         Constraint::Percentage(42),
            ].as_ref())
            .split(area) else { return Ok(()) };

        let (prev_items, prev_state) = self.stack.others.last_mut().unwrap();
        let prev_items = prev_items.to_listitems(false);
        prev_state.content_len(Some(prev_items.len() as u16));
        prev_state.viewport_len(Some(previous_area.height));
        let (current_items, current_state) = &mut self.stack.current;
        let current_items = if let CurrentPosition::Song(_) = self.position {
            current_items.to_listitems(true)
        } else {
            current_items.to_listitems(false)
        };
        current_state.content_len(Some(current_items.len() as u16));
        current_state.viewport_len(Some(current_area.height));

        let previous = List::new(prev_items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
        let current = List::new(current_items)
            .block(Block::default().borders(Borders::TOP | Borders::BOTTOM))
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
        let preview = List::new(self.next.clone())
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());

        let previous_scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .track_symbol(Some("│"))
            .end_symbol(Some("↓"))
            .track_style(Style::default().fg(Color::White).bg(Color::Black))
            .begin_style(Style::default().fg(Color::White).bg(Color::Black))
            .end_style(Style::default().fg(Color::White).bg(Color::Black))
            .thumb_style(Style::default().fg(Color::Blue));
        let current_scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalLeft)
            .begin_symbol(Some("↑"))
            .track_symbol(Some("│"))
            .end_symbol(Some("↓"))
            .track_style(Style::default().fg(Color::White).bg(Color::Black))
            .begin_style(Style::default().fg(Color::White).bg(Color::Black))
            .end_style(Style::default().fg(Color::White).bg(Color::Black))
            .thumb_style(Style::default().fg(Color::Blue));

        frame.render_stateful_widget(previous, previous_area, &mut prev_state.inner);
        frame.render_stateful_widget(current, current_area, &mut current_state.inner);
        frame.render_stateful_widget(
            previous_scrollbar,
            previous_area.inner(&ratatui::prelude::Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut prev_state.scrollbar_state,
        );
        frame.render_widget(preview, preview_area);
        frame.render_stateful_widget(
            current_scrollbar,
            preview_area.inner(&ratatui::prelude::Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut current_state.scrollbar_state,
        );

        Ok(())
    }

    async fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        let result = _client.list_tag("artist", None).await?.unwrap();
        self.stack = DirStack::new(result.0);
        self.position = CurrentPosition::default();
        self.next = self.prepare_preview(_client).await.unwrap();

        Ok(())
    }

    async fn handle_key(
        &mut self,
        key: KeyEvent,
        _client: &mut Client<'_>,
        _app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<Render, MpdError> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.stack.next();
                self.next = self.prepare_preview(_client).await.unwrap();
                return Ok(Render::No);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.stack.prev();
                self.next = self.prepare_preview(_client).await.unwrap();
                return Ok(Render::No);
            }
            KeyCode::Char('h') => {
                self.stack.pop();
                self.position = match &mut self.position {
                    CurrentPosition::Artist(val) => CurrentPosition::Artist(val.prev()),
                    CurrentPosition::Album(val) => CurrentPosition::Artist(val.prev()),
                    CurrentPosition::Song(val) => CurrentPosition::Album(val.prev()),
                };
                self.next = self.prepare_preview(_client).await.unwrap();
                return Ok(Render::No);
            }
            KeyCode::Char('l') | KeyCode::Enter => {
                let current = self.stack.current.0[self.stack.current.1.get_selected().unwrap()].clone();
                self.position = match &mut self.position {
                    CurrentPosition::Artist(val) => {
                        self.stack.push(val.fetch(_client, &current).await.unwrap());
                        CurrentPosition::Album(val.next(current))
                    }
                    CurrentPosition::Album(val) => {
                        self.stack.push(val.fetch(_client, &current).await.unwrap());
                        CurrentPosition::Song(val.next(current))
                    }
                    CurrentPosition::Song(val) => {
                        val.add_to_queue(_client, &current).await.unwrap();
                        _shared.status_message = Some(StatusMessage::new(
                            format!("'{}' by '{}' added to queue", current, val.values.artist),
                            Level::Info,
                        ));
                        CurrentPosition::Song(val.next())
                    }
                };
                self.next = self.prepare_preview(_client).await.unwrap();

                return Ok(Render::No);
            }
            _ => {}
        };
        Ok(Render::Yes)
    }
}

#[derive(Debug)]
struct Artist;
#[derive(Debug)]
struct Album {
    artist: String,
}
#[derive(Debug)]
struct Song {
    artist: String,
    album: String,
}
#[derive(Debug)]
struct Position<T> {
    values: T,
}
#[derive(Debug)]
enum CurrentPosition {
    Artist(Position<Artist>),
    Album(Position<Album>),
    Song(Position<Song>),
}

impl Default for CurrentPosition {
    fn default() -> Self {
        Self::Artist(Position { values: Artist })
    }
}

impl Position<Artist> {
    fn next(&self, artist: String) -> Position<Album> {
        Position {
            values: Album { artist },
        }
    }

    fn prev(&mut self) -> Position<Artist> {
        Position { values: Artist }
    }

    async fn fetch(&self, _client: &mut Client<'_>, value: &str) -> Result<Vec<String>> {
        Ok(_client
            .list_tag("album", Some(&[Filter { tag: "artist", value }]))
            .await?
            .unwrap()
            .0)
    }
}
impl Position<Album> {
    fn next(&mut self, album: String) -> Position<Song> {
        Position {
            values: Song {
                artist: std::mem::take(&mut self.values.artist),
                album,
            },
        }
    }

    fn prev(&mut self) -> Position<Artist> {
        Position { values: Artist }
    }

    async fn fetch(&self, _client: &mut Client<'_>, value: &str) -> Result<Vec<String>> {
        Ok(_client
            .list_tag(
                "title",
                Some(&[
                    Filter {
                        tag: "artist",
                        value: &self.values.artist,
                    },
                    Filter { tag: "album", value },
                ]),
            )
            .await?
            .unwrap()
            .0)
    }
}

impl Position<Song> {
    fn next(&mut self) -> Position<Song> {
        Position {
            values: Song {
                artist: std::mem::take(&mut self.values.artist),
                album: std::mem::take(&mut self.values.album),
            },
        }
    }
    fn prev(&mut self) -> Position<Album> {
        Position {
            values: Album {
                artist: std::mem::take(&mut self.values.artist),
            },
        }
    }

    async fn fetch(&self, _client: &mut Client<'_>, value: &str) -> Result<MpdSong> {
        let mut res = _client
            .find(&[
                Filter { tag: "title", value },
                Filter {
                    tag: "artist",
                    value: &self.values.artist,
                },
                Filter {
                    tag: "album",
                    value: &self.values.album,
                },
            ])
            .await?
            .unwrap()
            .0;
        Ok(std::mem::take(&mut res[0]))
    }

    async fn add_to_queue(&self, _client: &mut Client<'_>, value: &str) -> Result<()> {
        Ok(_client
            .find_add(&[
                Filter { tag: "title", value },
                Filter {
                    tag: "artist",
                    value: &self.values.artist,
                },
                Filter {
                    tag: "album",
                    value: &self.values.album,
                },
            ])
            .await?)
    }
}

pub trait SongOrTagExt {
    fn to_listitems(&self, is_song: bool) -> Vec<ListItem<'static>>;
}

impl SongOrTagExt for Vec<String> {
    fn to_listitems(&self, is_song: bool) -> Vec<ListItem<'static>> {
        self.iter()
            .map(|val| ListItem::new(format!(" {} {val}", if is_song { "🎵" } else { "📁" })))
            .collect::<Vec<ListItem>>()
    }
}