use std::cmp::Ordering;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ratatui::{
    prelude::{Backend, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Scrollbar, ScrollbarOrientation},
    Frame,
};
use tracing::instrument;

use crate::{
    mpd::{
        client::Client,
        commands::{lsinfo::FileOrDir, Song},
        mpd_client::MpdClient,
    },
    state::State,
    ui::{Level, Render, SharedUiState, StatusMessage},
    utils::macros::try_ret,
};

use super::{dirstack::DirStack, Screen};

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
    type Actions = DirectoriesActions;
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

        let preview = List::new(self.next.clone())
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
        frame.render_widget(preview, preview_area);

        {
            let (prev_items, prev_state) = self.dirs.previous();
            let prev_items = prev_items.to_listitems();
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
            let (current_items, current_state) = &mut self.dirs.current();
            let current_items = current_items.to_listitems();
            current_state.content_len(Some(u16::try_from(current_items.len())?));
            current_state.viewport_len(Some(current_area.height));

            let current = List::new(current_items)
                .block(Block::default().borders(Borders::TOP | Borders::BOTTOM))
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

    async fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        self.dirs = DirStack::new(_client.lsinfo(None).await?.0);
        self.prepare_preview(_client).await?;

        Ok(())
    }

    #[instrument(skip_all)]
    async fn handle_key(
        &mut self,
        action: Self::Actions,
        _client: &mut Client<'_>,
        _app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<Render> {
        match action {
            DirectoriesActions::Down => {
                self.dirs.next();
                self.prepare_preview(_client).await?;
            }
            DirectoriesActions::Up => {
                self.dirs.prev();
                self.prepare_preview(_client).await?;
            }
            DirectoriesActions::DownHalf => {
                self.dirs.next_half_viewport();
                self.prepare_preview(_client).await?;
            }
            DirectoriesActions::UpHalf => {
                self.dirs.prev_half_viewport();
                self.prepare_preview(_client).await?;
            }
            DirectoriesActions::AddAll => match self.dirs.get_selected() {
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
                            song.title.as_ref().map_or("Untitled", |v| v.as_str()),
                            song.artist.as_ref().map_or("Unknown", |v| v.as_str()),
                        ),
                        Level::Info,
                    ));
                }
                None => {}
            },
            DirectoriesActions::Enter => match self.dirs.get_selected() {
                Some(FileOrDir::Dir(dir)) => {
                    let new_current = _client.lsinfo(Some(&dir.full_path)).await?.0;
                    self.dirs.push(new_current);

                    self.prepare_preview(_client).await?;
                }
                Some(FileOrDir::File(song)) => {
                    _client.add(&song.file).await?;
                    _shared.status_message = Some(StatusMessage::new(
                        format!(
                            "'{}' by '{}' added to queue",
                            song.title.as_ref().map_or("Untitled", |v| v.as_str()),
                            song.artist.as_ref().map_or("Unknown", |v| v.as_str()),
                        ),
                        Level::Info,
                    ));
                }
                None => {}
            },
            DirectoriesActions::Leave => {
                self.dirs.pop();
                self.prepare_preview(_client).await?;
            }
        }
        return Ok(Render::Yes);
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum DirectoriesActions {
    Down,
    Up,
    DownHalf,
    UpHalf,
    Enter,
    Leave,
    AddAll,
}

impl DirectoriesScreen {
    #[instrument(err, skip(client))]
    async fn prepare_preview(&mut self, client: &mut Client<'_>) -> Result<()> {
        if let Some(idx) = self.dirs.current().1.get_selected() {
            match &self.dirs.current().0[idx] {
                FileOrDir::Dir(dir) => {
                    let mut res = try_ret!(
                        client.lsinfo(Some(&dir.full_path)).await,
                        "Failed to get lsinfo for dir"
                    )
                    .0;
                    res.sort();
                    self.next = res.to_listitems();
                }
                FileOrDir::File(song) => self.next = song.to_listitems(),
            }
        } else {
            tracing::error!("Failed to get selected item because none was selected");
            return Err(anyhow!("Failed to get selected item because none was selected"));
        }
        Ok(())
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
                    FileOrDir::Dir(v) => (" 📁", v.path.clone()),
                    FileOrDir::File(v) => (" 🎵", v.title.as_ref().map_or("Untitled", |v| v.as_str()).to_owned()),
                };
                ListItem::new(format!("{kind} {name}"))
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
            Span::from(self.title.as_ref().map_or("Untitled", |v| v.as_str()).to_owned()),
        ]);
        let artist = Line::from(vec![
            start_of_line_spacer.clone(),
            Span::styled("Artist", key_style),
            separator.clone(),
            Span::from(": "),
            Span::from(self.artist.as_ref().map_or("Unknown", |v| v.as_str()).to_owned()),
        ]);
        let album = Line::from(vec![
            start_of_line_spacer.clone(),
            Span::styled("Album", key_style),
            separator.clone(),
            Span::from(self.album.as_ref().map_or("Unknown", |v| v.as_str()).to_owned()),
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
                        Span::styled(k.clone(), key_style),
                        separator.clone(),
                        Span::from(v.clone()),
                    ])
                })
                .collect(),
        ]
        .concat();

        r.into_iter().map(ListItem::new).collect()
    }
}
