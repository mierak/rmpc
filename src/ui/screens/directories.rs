use std::cmp::Ordering;

use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::{Backend, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Scrollbar, ScrollbarOrientation},
    Frame,
};

use crate::{
    mpd::{
        client::Client,
        commands::{lsinfo::FileOrDir, Song},
        errors::MpdError,
    },
    state::State,
    ui::{Level, Render, SharedUiState, StatusMessage},
};

use super::{DirStack, Screen};

#[derive(Debug)]
pub struct DirectoriesScreen {
    dirs: DirStack<FileOrDir>,
    next: Vec<ListItem<'static>>,
}

impl Default for DirectoriesScreen {
    fn default() -> Self {
        Self {
            dirs: DirStack::new(Vec::new()),
            next: Vec::new(),
        }
    }
}

#[async_trait]
impl Screen for DirectoriesScreen {
    fn render<B: Backend>(
        &mut self,
        frame: &mut Frame<B>,
        area: Rect,
        _app: &mut crate::state::State,
        _state: &mut SharedUiState,
    ) -> anyhow::Result<()> {
        let [previous_area, current_area, preview_area] = *Layout::default()
            .direction(ratatui::prelude::Direction::Horizontal)
            // cfg
            .constraints([
                         Constraint::Percentage(20),
                         Constraint::Percentage(38),
                         Constraint::Percentage(42),
            ].as_ref())
            .split(area) else { return Ok(()) };

        let (prev_items, prev_state) = self.dirs.others.last_mut().unwrap();
        let prev_items = prev_items.to_listitems();
        prev_state.content_len(Some(prev_items.len() as u16));
        prev_state.viewport_len(Some(previous_area.height));
        let (current_items, current_state) = &mut self.dirs.current;
        let current_items = current_items.to_listitems();
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
        self.dirs = DirStack::new(_client.lsinfo(None).await.unwrap().0);
        self.prepare_preview(_client).await;

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
                self.dirs.next();
                self.prepare_preview(_client).await;
                return Ok(Render::No);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.dirs.prev();
                self.prepare_preview(_client).await;
                return Ok(Render::No);
            }
            KeyCode::Char('a') => match self.dirs.get_selected() {
                Some(FileOrDir::Dir(dir)) => {
                    _client.add(&dir.full_path).await?;
                    _shared.status_message = Some(StatusMessage::new(
                        format!("Directory '{}' added to queue", dir.full_path),
                        Level::Info,
                    ));
                }
                Some(FileOrDir::File(song)) => {
                    _client.add(&song.file).await?;
                    _shared.status_message = Some(StatusMessage::new(
                        format!(
                            "'{}' by '{}' added to queue",
                            song.title.as_ref().unwrap_or(&"Untilted".to_owned()),
                            song.artist.as_ref().unwrap_or(&"Unknown".to_owned())
                        ),
                        Level::Info,
                    ));
                }
                None => {}
            },
            KeyCode::Enter | KeyCode::Char('l') => {
                match self.dirs.get_selected() {
                    Some(FileOrDir::Dir(dir)) => {
                        let new_current = _client.lsinfo(Some(&dir.full_path)).await.unwrap().0;
                        self.dirs.push(new_current);

                        self.prepare_preview(_client).await;
                    }
                    Some(FileOrDir::File(song)) => {
                        _client.add(&song.file).await.unwrap();
                        _shared.status_message = Some(StatusMessage::new(
                            format!(
                                "'{}' by '{}' added to queue",
                                song.title.as_ref().unwrap_or(&"Untilted".to_owned()),
                                song.artist.as_ref().unwrap_or(&"Unknown".to_owned())
                            ),
                            Level::Info,
                        ));
                    }
                    None => {}
                }
                return Ok(Render::No);
            }
            KeyCode::Char('h') => {
                self.dirs.pop();
                self.prepare_preview(_client).await;
                return Ok(Render::No);
            }
            _ => {}
        }
        Ok(Render::Yes)
    }
}

impl DirectoriesScreen {
    async fn prepare_preview(&mut self, client: &mut Client<'_>) {
        let a = &self.dirs.current;
        match &a.0[a.1.get_selected().unwrap()] {
            FileOrDir::Dir(dir) => {
                let mut res = client.lsinfo(Some(&dir.full_path)).await.unwrap().0;
                res.sort();
                self.next = res.to_listitems();
            }
            FileOrDir::File(song) => self.next = song.to_listitems(),
        }
    }
}

impl std::cmp::Ord for FileOrDir {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (_, FileOrDir::Dir(_)) => Ordering::Greater,
            (FileOrDir::Dir(_), _) => Ordering::Less,
            (FileOrDir::File(Song { title: t1, .. }), FileOrDir::File(Song { title: t2, .. })) => t1.cmp(t2),
        }
    }
}
impl std::cmp::PartialOrd for FileOrDir {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (_, FileOrDir::Dir(_)) => Some(Ordering::Greater),
            (FileOrDir::Dir(_), _) => Some(Ordering::Less),
            (FileOrDir::File(Song { title: t1, .. }), FileOrDir::File(Song { title: t2, .. })) => Some(t1.cmp(t2)),
        }
    }
}

pub trait FileOrDirExt {
    fn to_listitems(&self) -> Vec<ListItem<'static>>;
}

impl FileOrDirExt for Vec<FileOrDir> {
    fn to_listitems(&self) -> Vec<ListItem<'static>> {
        self.iter()
            .map(|val| {
                let (kind, name) = match val {
                    // cfg
                    FileOrDir::Dir(v) => (" 📁", v.path.clone().to_owned()),
                    FileOrDir::File(v) => (" 🎵", v.title.as_ref().unwrap_or(&"Untitled".to_string()).to_owned()),
                };
                ListItem::new(format!("{} {}", kind, name))
            })
            .collect::<Vec<ListItem>>()
    }
}

impl FileOrDirExt for Song {
    fn to_listitems(&self) -> Vec<ListItem<'static>> {
        let key_style = Style::default().fg(Color::Yellow);
        let separator = Span::from(": ");
        let start_of_line_spacer = Span::from(" ");

        let title = Line::from(vec![
            start_of_line_spacer.clone(),
            Span::styled("Title", key_style),
            separator.clone(),
            Span::from(self.title.as_ref().unwrap_or(&"Untitled".to_owned()).to_owned()),
        ]);
        let artist = Line::from(vec![
            start_of_line_spacer.clone(),
            Span::styled("Artist", key_style),
            separator.clone(),
            Span::from(": "),
            Span::from(self.artist.as_ref().unwrap_or(&"Unknown".to_owned()).to_owned()),
        ]);
        let album = Line::from(vec![
            start_of_line_spacer.clone(),
            Span::styled("Album", key_style),
            separator.clone(),
            Span::from(self.album.as_ref().unwrap_or(&"Unknown".to_owned()).to_owned()),
        ]);
        let duration = Line::from(vec![
            start_of_line_spacer.clone(),
            Span::styled("Duration", key_style),
            separator.clone(),
            Span::from(
                self.duration
                    .as_ref()
                    .map_or("-".to_owned(), |v| v.as_secs().to_string()),
            ),
        ]);
        let r = vec![title, artist, album, duration];
        let r = [
            r,
            self.others
                .iter()
                .map(|(k, v)| {
                    Line::from(vec![
                        start_of_line_spacer.clone(),
                        Span::styled(k.to_owned(), key_style),
                        separator.clone(),
                        Span::from(v.to_owned()),
                    ])
                })
                .collect(),
        ]
        .concat();

        r.into_iter().map(ListItem::new).collect()
    }
}
