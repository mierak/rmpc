use std::cmp::Ordering;

use anyhow::{Context, Result};
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::{Backend, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
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
    ui::{KeyHandleResult, Level, SharedUiState, StatusMessage},
};

use super::{dirstack::DirStack, CommonAction, Screen, ToListItems};

#[derive(Debug)]
pub struct DirectoriesScreen {
    stack: DirStack<FileOrDir>,
    next: Vec<ListItem<'static>>,
    filter_input_mode: bool,
}

impl Default for DirectoriesScreen {
    fn default() -> Self {
        Self {
            stack: DirStack::new(Vec::new()),
            next: Vec::new(),
            filter_input_mode: false,
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
        app: &mut crate::state::State,
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
            let (prev_items, prev_state) = self.stack.previous();
            let prev_items = prev_items.to_listitems(app);
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
        let title = self.stack.filter.as_ref().map(|v| format!("[FILTER]: {v} "));
        {
            let (current_items, current_state) = &mut self.stack.current();
            let current_items = current_items.to_listitems(app);
            current_state.content_len(Some(u16::try_from(current_items.len())?));
            current_state.viewport_len(Some(current_area.height));

            let current = List::new(current_items)
                .block({
                    let mut b = Block::default().borders(Borders::TOP | Borders::BOTTOM);
                    if let Some(ref title) = title {
                        b = b.title(title.blue());
                    }
                    b
                })
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
        client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        self.stack = DirStack::new(client.lsinfo(None).await?.0);
        self.next = self
            .prepare_preview(client, _app)
            .await
            .context("Cannot prepare preview")?;

        Ok(())
    }

    #[instrument(skip_all)]
    async fn handle_action(
        &mut self,
        event: KeyEvent,
        client: &mut Client<'_>,
        app: &mut State,
        shared: &mut SharedUiState,
    ) -> Result<KeyHandleResult> {
        if self.filter_input_mode {
            match event.code {
                KeyCode::Char(c) => {
                    if let Some(ref mut f) = self.stack.filter {
                        f.push(c);
                    };
                    Ok(KeyHandleResult::RenderRequested)
                }
                KeyCode::Backspace => {
                    if let Some(ref mut f) = self.stack.filter {
                        f.pop();
                    };
                    Ok(KeyHandleResult::RenderRequested)
                }
                KeyCode::Enter => {
                    self.filter_input_mode = false;
                    self.stack.jump_forward();
                    Ok(KeyHandleResult::RenderRequested)
                }
                KeyCode::Esc => {
                    self.filter_input_mode = false;
                    self.stack.filter = None;
                    Ok(KeyHandleResult::RenderRequested)
                }
                _ => Ok(KeyHandleResult::SkipRender),
            }
        } else if let Some(action) = app.config.keybinds.directories.get(&event.into()) {
            match action {
                DirectoriesActions::AddAll => {
                    match self.stack.get_selected() {
                        Some(FileOrDir::Dir(dir)) => {
                            client.add(&dir.full_path).await?;
                            shared.status_message = Some(StatusMessage::new(
                                format!("Directory '{}' added to queue", dir.full_path),
                                Level::Info,
                            ));
                        }
                        Some(FileOrDir::File(song)) => {
                            client.add(&song.file).await?;
                            shared.status_message = Some(StatusMessage::new(
                                format!(
                                    "'{}' by '{}' added to queue",
                                    song.title.as_ref().map_or("Untitled", |v| v.as_str()),
                                    song.artist.as_ref().map_or("Unknown", |v| v.as_str()),
                                ),
                                Level::Info,
                            ));
                        }
                        None => {}
                    };
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
                    self.next = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Top => {
                    self.stack.first();
                    self.next = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Right => {
                    match self.stack.get_selected() {
                        Some(FileOrDir::Dir(dir)) => {
                            let new_current = client.lsinfo(Some(&dir.full_path)).await?.0;
                            self.stack.push(new_current);

                            self.next = self
                                .prepare_preview(client, app)
                                .await
                                .context("Cannot prepare preview")?;
                        }
                        Some(FileOrDir::File(song)) => {
                            client.add(&song.file).await?;
                            shared.status_message = Some(StatusMessage::new(
                                format!(
                                    "'{}' by '{}' added to queue",
                                    song.title.as_ref().map_or("Untitled", |v| v.as_str()),
                                    song.artist.as_ref().map_or("Unknown", |v| v.as_str()),
                                ),
                                Level::Info,
                            ));
                        }
                        None => {}
                    };
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::Left => {
                    self.stack.pop();
                    self.next = self
                        .prepare_preview(client, app)
                        .await
                        .context("Cannot prepare preview")?;
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::EnterSearch => {
                    self.filter_input_mode = true;
                    self.stack.filter = Some(String::new());
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::NextResult => {
                    self.stack.jump_forward();
                    Ok(KeyHandleResult::RenderRequested)
                }
                CommonAction::PreviousResult => {
                    self.stack.jump_back();
                    Ok(KeyHandleResult::RenderRequested)
                }
            }
        } else {
            Ok(KeyHandleResult::KeyNotHandled)
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum DirectoriesActions {
    AddAll,
}

impl DirectoriesScreen {
    #[instrument(skip(client))]
    async fn prepare_preview(&mut self, client: &mut Client<'_>, state: &State) -> Option<Vec<ListItem<'static>>> {
        let idx = self.stack.current().1.get_selected()?;
        match &self.stack.current().0[idx] {
            FileOrDir::Dir(dir) => {
                let mut res = match client.lsinfo(Some(&dir.full_path)).await {
                    Ok(val) => val,
                    Err(err) => {
                        tracing::error!(message = "Failed to get lsinfo for dir", error = ?err);
                        return None;
                    }
                }
                .0;
                res.sort();
                Some(res.to_listitems(state))
            }
            FileOrDir::File(song) => Some(song.to_listitems(state)),
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

impl ToListItems for Vec<FileOrDir> {
    fn to_listitems(&self, state: &State) -> Vec<ListItem<'static>> {
        self.iter()
            .map(|val| {
                let (kind, name) = match val {
                    // cfg
                    FileOrDir::Dir(v) => (state.config.symbols.dir, v.path.clone()),
                    FileOrDir::File(v) => (
                        state.config.symbols.song,
                        v.title.as_ref().map_or("Untitled", |v| v.as_str()).to_owned(),
                    ),
                };
                ListItem::new(format!("{kind} {name}"))
            })
            .collect::<Vec<ListItem>>()
    }
}
