use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::{Backend, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, List, ListItem, ListState, Scrollbar, ScrollbarOrientation},
    Frame,
};

use crate::{
    mpd::{
        client::Client,
        commands::{lsinfo::FileOrDir, Song},
        errors::MpdError,
    },
    state::State,
    ui::{Level, MyState, Render, SharedUiState, StatusMessage},
};

use super::Screen;

#[derive(Default, Debug)]
pub struct DirectoriesScreen {
    dirs: DirStack,
    next: Vec<ListItem<'static>>,
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
            .track_symbol("│")
            .end_symbol(Some("↓"))
            .track_style(Style::default().fg(Color::White).bg(Color::Black))
            .begin_style(Style::default().fg(Color::White).bg(Color::Black))
            .end_style(Style::default().fg(Color::White).bg(Color::Black))
            .thumb_style(Style::default().fg(Color::Blue));
        let current_scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalLeft)
            .begin_symbol(Some("↑"))
            .track_symbol("│")
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
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.dirs.prev();
                self.prepare_preview(_client).await;
                return Ok(Render::NoSkip);
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
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('h') => {
                self.dirs.pop();
                self.prepare_preview(_client).await;
                return Ok(Render::NoSkip);
            }
            _ => {}
        }
        Ok(Render::Skip)
    }
}

#[derive(Default, Debug)]
struct DirStack {
    current: (Vec<FileOrDir>, MyState<ListState>),
    others: Vec<(Vec<FileOrDir>, MyState<ListState>)>,
}

impl DirStack {
    fn new(mut root: Vec<FileOrDir>) -> Self {
        let mut val = DirStack::default();
        let mut root_state = MyState::default();

        val.push(Vec::new());

        if !root.is_empty() {
            root_state.select(Some(0));
            root.sort();
        };

        val.current = (root, root_state);
        val
    }

    fn push(&mut self, head: Vec<FileOrDir>) {
        let mut new_state = MyState::default();
        if !head.is_empty() {
            new_state.select(Some(0));
        };
        let current_head = std::mem::replace(&mut self.current, (head, new_state));
        self.others.push(current_head);
    }

    fn pop(&mut self) -> Option<(Vec<FileOrDir>, MyState<ListState>)> {
        if self.others.len() > 1 {
            let top = self.others.pop().expect("There should always be at least two elements");
            Some(std::mem::replace(&mut self.current, top))
        } else {
            None
        }
    }

    fn get_selected(&self) -> Option<&FileOrDir> {
        if let Some(sel) = self.current.1.get_selected() {
            self.current.0.get(sel)
        } else {
            None
        }
    }

    fn next(&mut self) {
        self.current.1.next()
    }

    fn prev(&mut self) {
        self.current.1.prev()
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

trait FileOrDirExt {
    fn to_listitems(&self) -> Vec<ListItem<'static>>;
}

impl FileOrDirExt for Vec<FileOrDir> {
    fn to_listitems(&self) -> Vec<ListItem<'static>> {
        self.iter()
            .map(|val| {
                let (kind, name) = match val {
                    // cfg
                    FileOrDir::Dir(v) => (" 📁", v.path.clone().to_owned()),
                    FileOrDir::File(v) => (" 🎶", v.title.as_ref().unwrap_or(&"Untitled".to_string()).to_owned()),
                };
                ListItem::new(format!("{} {}", kind, name))
            })
            .collect::<Vec<ListItem>>()
    }
}

impl FileOrDirExt for Song {
    fn to_listitems(&self) -> Vec<ListItem<'static>> {
        let mut res = vec![
            ListItem::new(format!(
                " {}: {}",
                "Title",
                self.title.as_ref().unwrap_or(&"Untitled".to_owned())
            )),
            ListItem::new(format!(
                " {}: {}",
                "Artist",
                self.artist.as_ref().unwrap_or(&"Unknown".to_owned())
            )),
            ListItem::new(format!(
                " {}: {}",
                "Unknown Album",
                self.album.as_ref().unwrap_or(&"Untitled".to_owned())
            )),
            ListItem::new(format!(" {}: {}", "File", self.file)),
            ListItem::new(format!(
                " {}: {}",
                "Duration",
                self.duration
                    .as_ref()
                    .map_or("-".to_owned(), |v| v.as_secs().to_string())
            )),
        ];
        let mut s = self
            .others
            .iter()
            .map(|v| ListItem::new(format!(" {}: {}", v.0, v.1)))
            .collect::<Vec<ListItem>>();
        res.append(&mut s);
        res
    }
}
