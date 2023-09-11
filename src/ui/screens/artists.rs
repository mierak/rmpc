use crate::{
    mpd::{
        client::Client,
        commands::Song as MpdSong,
        mpd_client::{Filter, MpdClient},
    },
    state::State,
    ui::{KeyHandleResult, Level, SharedUiState, StatusMessage},
};

use super::{dirstack::DirStack, CommonAction, Screen, SongOrTagExt, ToListItems};
use anyhow::{Context, Result};
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::Rect, widgets::ListItem, Frame};
use ratatui::{
    prelude::{Constraint, Layout},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, List, Scrollbar, ScrollbarOrientation},
};
use tracing::instrument;

#[derive(Debug)]
pub struct ArtistsScreen {
    stack: DirStack<String>,
    position: CurrentPosition,
    next: Vec<ListItem<'static>>,
    filter_input_mode: bool,
}

impl Default for ArtistsScreen {
    fn default() -> Self {
        Self {
            stack: DirStack::new(Vec::new()),
            position: CurrentPosition::Artist(Position { values: Artist }),
            next: Vec::new(),
            filter_input_mode: false,
        }
    }
}

impl ArtistsScreen {
    async fn prepare_preview(&mut self, client: &mut Client<'_>, state: &State) -> Result<Vec<ListItem<'static>>> {
        let idx = self
            .stack
            .current()
            .1
            .get_selected()
            .context("Expected an item to be selected")?;
        let current = &self.stack.current().0[idx];
        Ok(match &self.position {
            CurrentPosition::Artist(val) => val.fetch(client, current).await?.to_listitems(false, state),
            CurrentPosition::Album(val) => val.fetch(client, current).await?.to_listitems(true, state),
            CurrentPosition::Song(val) => val.fetch(client, current).await?.to_listitems(state),
        })
    }
}

#[async_trait]
impl Screen for ArtistsScreen {
    type Actions = ArtistsActions;
    fn render<B: ratatui::prelude::Backend>(
        &mut self,
        frame: &mut Frame<B>,
        area: Rect,
        app: &mut State,
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

        let preview = List::new(self.next.clone())
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
        frame.render_widget(preview, preview_area);

        {
            let (prev_items, prev_state) = self.stack.previous();
            let prev_items = prev_items.to_listitems(false, app);
            prev_state.content_len(Some(u16::try_from(prev_items.len())?));
            prev_state.viewport_len(Some(previous_area.height));

            let previous = List::new(prev_items)
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

            frame.render_stateful_widget(previous, previous_area, &mut prev_state.inner);
            frame.render_stateful_widget(
                previous_scrollbar,
                previous_area.inner(&ratatui::prelude::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut prev_state.scrollbar_state,
            );
        }
        {
            let (current_items, current_state) = self.stack.current();
            let current_items = if let CurrentPosition::Song(_) = self.position {
                current_items.to_listitems(true, app)
            } else {
                current_items.to_listitems(false, app)
            };
            current_state.content_len(Some(u16::try_from(current_items.len())?));
            current_state.viewport_len(Some(current_area.height));

            let current = List::new(current_items)
                .block(
                    Block::default().borders(Borders::TOP | Borders::BOTTOM), // .title(current_state.filter.as_ref().map_or(String::new(), Clone::clone)),
                )
                .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
            let current_scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalLeft)
                .begin_symbol(Some("↑"))
                .track_symbol(Some("│"))
                .end_symbol(Some("↓"))
                .track_style(Style::default().fg(Color::White).bg(Color::Black))
                .begin_style(Style::default().fg(Color::White).bg(Color::Black))
                .end_style(Style::default().fg(Color::White).bg(Color::Black))
                .thumb_style(Style::default().fg(Color::Blue));

            frame.render_stateful_widget(current, current_area, &mut current_state.inner);
            frame.render_stateful_widget(
                current_scrollbar,
                preview_area.inner(&ratatui::prelude::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut current_state.scrollbar_state,
            );
        }

        Ok(())
    }

    #[instrument(err)]
    async fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        match _client.list_tag("artist", None).await.context("Cannot list artists")? {
            Some(result) => {
                self.stack = DirStack::new(result.0);
                self.position = CurrentPosition::default();
                self.next = self
                    .prepare_preview(_client, _app)
                    .await
                    .context("Cannot prepare preview")?;
            }
            None => {
                _shared.status_message = Some(StatusMessage::new("No artists found!".to_owned(), Level::Info));
            }
        };

        Ok(())
    }

    #[instrument(err)]
    async fn handle_action(
        &mut self,
        event: KeyEvent,
        client: &mut Client<'_>,
        app: &mut State,
        shared: &mut SharedUiState,
    ) -> Result<KeyHandleResult> {
        if self.filter_input_mode {
            match event.code {
                // KeyCode::Char(c) => {
                //     if let Some(ref mut f) = self.stack.current().1.filter {
                //         f.push(c);
                //     } else {
                //         self.stack.current().1.filter = Some(String::new());
                //     };
                //     Ok(KeyHandleResult::RenderRequested)
                // }
                // KeyCode::Backspace => {
                //     if let Some(ref mut f) = self.stack.current().1.filter {
                //         f.pop();
                //     };
                //     Ok(KeyHandleResult::RenderRequested)
                // }
                // KeyCode::Enter => {
                //     self.filter_input_mode = false;
                //     Ok(KeyHandleResult::RenderRequested)
                // }
                // KeyCode::Esc => {
                //     self.filter_input_mode = false;
                //     self.stack.current().1.filter = None;
                //     Ok(KeyHandleResult::RenderRequested)
                // }
                _ => Ok(KeyHandleResult::SkipRender),
            }
        } else if let Some(action) = app.config.keybinds.artists.get(&event.into()) {
            match action {
                ArtistsActions::EnterSearch => {
                    self.filter_input_mode = true;
                    Ok(KeyHandleResult::RenderRequested)
                }
                ArtistsActions::LeaveSearch => {
                    self.filter_input_mode = false;
                    Ok(KeyHandleResult::RenderRequested)
                }
            }
        } else if let Some(action) = app.config.keybinds.navigation.get(&event.into()) {
            match action {
                CommonAction::DownHalf => {
                    self.stack.next_half_viewport();
                    self.next = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::UpHalf => {
                    self.stack.prev_half_viewport();
                    self.next = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Up => {
                    self.stack.prev();
                    self.next = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Down => {
                    self.stack.next();
                    self.next = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Bottom => {
                    self.stack.last();
                    self.prepare_preview(client, app).await?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Top => {
                    self.stack.first();
                    self.prepare_preview(client, app).await?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Right => {
                    let idx = self
                        .stack
                        .current()
                        .1
                        .get_selected()
                        .context("Expected an item to be selected")?;
                    let current = self.stack.current().0[idx].clone();
                    self.position = match &mut self.position {
                        CurrentPosition::Artist(val) => {
                            self.stack.push(val.fetch(client, &current).await?);
                            CurrentPosition::Album(val.next(current))
                        }
                        CurrentPosition::Album(val) => {
                            self.stack.push(val.fetch(client, &current).await?);
                            CurrentPosition::Song(val.next(current))
                        }
                        CurrentPosition::Song(val) => {
                            val.add_to_queue(client, &current).await?;
                            shared.status_message = Some(StatusMessage::new(
                                format!("'{}' by '{}' added to queue", current, val.values.artist),
                                Level::Info,
                            ));
                            CurrentPosition::Song(val.next())
                        }
                    };
                    self.next = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Left => {
                    self.stack.pop();
                    self.position = match &mut self.position {
                        CurrentPosition::Artist(val) => CurrentPosition::Artist(val.prev()),
                        CurrentPosition::Album(val) => CurrentPosition::Artist(val.prev()),
                        CurrentPosition::Song(val) => CurrentPosition::Album(val.prev()),
                    };
                    self.next = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
            }
        } else {
            Ok(KeyHandleResult::KeyNotHandled)
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum ArtistsActions {
    EnterSearch,
    LeaveSearch,
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

    async fn fetch(&self, client: &mut Client<'_>, value: &str) -> Result<Vec<String>> {
        Ok(client
            .list_tag("album", Some(&[Filter { tag: "artist", value }]))
            .await?
            .unwrap_or_else(|| crate::mpd::commands::list::MpdList(Vec::new()))
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

    async fn fetch(&self, client: &mut Client<'_>, value: &str) -> Result<Vec<String>> {
        Ok(client
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
            .unwrap_or_else(|| crate::mpd::commands::list::MpdList(Vec::new()))
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

    async fn fetch(&self, client: &mut Client<'_>, value: &str) -> Result<MpdSong> {
        let mut res = client
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
            .unwrap_or_else(|| crate::mpd::commands::Songs(Vec::new()))
            .0;
        Ok(std::mem::take(&mut res[0]))
    }

    async fn add_to_queue(&self, client: &mut Client<'_>, value: &str) -> Result<()> {
        Ok(client
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
