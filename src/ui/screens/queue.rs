use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use strum::Display;

use crate::{
    mpd::{client::Client, mpd_client::MpdClient},
    state::PlayListInfoExt,
    ui::{
        modals::{confirm_queue_clear::ConfirmQueueClearModal, save_queue::SaveQueueModal, Modals},
        utils::dirstack::DirState,
        widgets::kitty_image::{ImageState, KittyImage},
        DurationExt, KeyHandleResultInternal, SharedUiState,
    },
};
use ratatui::{
    prelude::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, Row, Scrollbar, ScrollbarOrientation, Table, TableState},
    Frame,
};
use tracing::error;

use crate::state::State;

use super::{CommonAction, Screen};

const TABLE_HEADER: &[&str] = &[" Artist", "Title", "Album", "Duration"];

#[derive(Debug, Default)]
pub struct QueueScreen {
    img_state: ImageState,
    scrolling_state: DirState<TableState>,
    filter: Option<String>,
    filter_input_mode: bool,
}

impl Screen for QueueScreen {
    type Actions = QueueActions;
    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> anyhow::Result<()> {
        let queue_len = app.queue.len().unwrap_or(0);
        let show_image = !app.config.disable_images;

        let [img_section, queue_section] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(if show_image { 35 } else { 0 }),
                    Constraint::Percentage(if show_image { 65 } else { 100 }),
                ]
                .as_ref(),
            )
            .split(area)
        else {
            return Ok(());
        };

        let [table_header_section, mut queue_section] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(2), Constraint::Percentage(100)].as_ref())
            .split(queue_section)
        else {
            return Ok(());
        };

        self.scrolling_state.set_viewport_len(Some(queue_section.height.into()));
        self.scrolling_state.set_content_len(Some(queue_len));
        if show_image {
            self.img_state.image(&mut app.album_art);
        }

        let mut rows = Vec::with_capacity(queue_len);
        if let Some(queue) = app.queue.as_ref() {
            for song in queue {
                let mut row = Row::new(vec![
                    song.artist.as_ref().map_or("-".to_owned(), |v| format!(" {v}")),
                    song.title.as_ref().map_or("-", |v| v).to_owned(),
                    song.album.as_ref().map_or("-", |v| v).to_owned(),
                    song.duration.as_ref().map_or("-".to_string(), DurationExt::to_string),
                ]);
                if app.status.songid.as_ref().is_some_and(|v| *v == song.id) {
                    row = row.style(Style::default().fg(Color::Blue));
                }
                rows.push(row);
            }
        }

        let column_widths = [
            Constraint::Percentage(15),
            Constraint::Percentage(35),
            Constraint::Percentage(35),
            Constraint::Percentage(15),
        ];
        let header_table = Table::new([], column_widths)
            .header(Row::new(TABLE_HEADER.to_vec()))
            .block(Block::default().borders(Borders::TOP));

        let title = self.filter.as_ref().map(|v| format!("[FILTER]: {v} "));
        let table = Table::new(rows, column_widths)
            .block({
                let mut b = Block::default().borders(Borders::TOP);
                if let Some(ref title) = title {
                    b = b.title(title.clone().blue());
                }
                b
            })
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .track_symbol(Some("│"))
            .end_symbol(Some("↓"))
            .track_style(Style::default().fg(Color::White).bg(Color::Black))
            .begin_style(Style::default().fg(Color::White).bg(Color::Black))
            .end_style(Style::default().fg(Color::White).bg(Color::Black))
            .thumb_style(Style::default().fg(Color::Blue));

        frame.render_widget(header_table, table_header_section);
        frame.render_stateful_widget(table, queue_section, self.scrolling_state.as_render_state_ref());

        queue_section.y = queue_section.y.saturating_add(1);
        queue_section.height = queue_section.height.saturating_sub(1);
        frame.render_stateful_widget(
            scrollbar,
            queue_section.inner(&ratatui::prelude::Margin {
                vertical: 0,
                horizontal: 0,
            }),
            self.scrolling_state.as_scrollbar_state_ref(),
        );
        if show_image {
            frame.render_stateful_widget(
                KittyImage::default().block(Block::default().borders(Borders::TOP)),
                img_section,
                &mut self.img_state,
            );
        }

        Ok(())
    }

    fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        if let Some(songid) = app.status.songid {
            let idx = app
                .queue
                .as_ref()
                .and_then(|queue| queue.iter().enumerate().find(|(_, song)| song.id == songid))
                .map(|v| v.0);
            self.scrolling_state.set_content_len(app.queue.len());
            self.scrolling_state.select(idx);
        }

        Ok(())
    }

    fn handle_action(
        &mut self,
        event: KeyEvent,
        client: &mut Client<'_>,
        app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<KeyHandleResultInternal> {
        if self.filter_input_mode {
            match event.code {
                KeyCode::Char(c) => {
                    if let Some(ref mut f) = self.filter {
                        f.push(c);
                    };
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Backspace => {
                    if let Some(ref mut f) = self.filter {
                        f.pop();
                    };
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Enter => {
                    self.filter_input_mode = false;
                    self.jump_forward(app);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Esc => {
                    self.filter_input_mode = false;
                    self.filter = None;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                _ => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else if let Some(action) = app.config.keybinds.queue.get(&event.into()) {
            match action {
                QueueActions::Delete => {
                    if let Some(selected_song) = app.queue.get_selected(self.scrolling_state.get_selected()) {
                        match client.delete_id(selected_song.id) {
                            Ok(()) => {}
                            Err(e) => error!("{:?}", e),
                        }
                    } else {
                        error!("No song selected");
                    }
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                QueueActions::DeleteAll => Ok(KeyHandleResultInternal::Modal(Some(Modals::ConfirmQueueClear(
                    ConfirmQueueClearModal::default(),
                )))),
                QueueActions::Play => {
                    if let Some(selected_song) = app.queue.get_selected(self.scrolling_state.get_selected()) {
                        client.play_id(selected_song.id)?;
                    }
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                QueueActions::Save => Ok(KeyHandleResultInternal::Modal(Some(Modals::SaveQueue(
                    SaveQueueModal::default(),
                )))),
            }
        } else if let Some(action) = app.config.keybinds.navigation.get(&event.into()) {
            match action {
                CommonAction::DownHalf => {
                    if !app.queue.is_empty_or_none() {
                        self.scrolling_state.next_half_viewport();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::UpHalf => {
                    if !app.queue.is_empty_or_none() {
                        self.scrolling_state.prev_half_viewport();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Up => {
                    if !app.queue.is_empty_or_none() {
                        self.scrolling_state.prev();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    if !app.queue.is_empty_or_none() {
                        self.scrolling_state.next();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Bottom => {
                    if !app.queue.is_empty_or_none() {
                        self.scrolling_state.last();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Top => {
                    if !app.queue.is_empty_or_none() {
                        self.scrolling_state.first();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Right => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Left => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::EnterSearch => {
                    self.filter_input_mode = true;
                    self.filter = Some(String::new());
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::NextResult => {
                    self.jump_forward(app);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::PreviousResult => {
                    self.jump_back(app);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Select => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Add => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Delete => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Rename => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

impl QueueScreen {
    pub fn jump_forward(&mut self, app: &mut crate::state::State) {
        if let Some(filter) = self.filter.as_ref() {
            if let Some(selected) = self.scrolling_state.get_selected() {
                for i in selected + 1..app.queue.len().unwrap_or(0) {
                    if app.queue.as_ref().is_some_and(|q| {
                        q[i].title
                            .as_ref()
                            .is_some_and(|v| v.to_lowercase().contains(&filter.to_lowercase()))
                    }) {
                        self.scrolling_state.select(Some(i));
                        break;
                    }
                }
            }
        }
    }

    pub fn jump_back(&mut self, app: &mut crate::state::State) {
        if let Some(filter) = self.filter.as_ref() {
            if let Some(selected) = self.scrolling_state.get_selected() {
                for i in (0..selected).rev() {
                    if app.queue.as_ref().is_some_and(|q| {
                        q[i].title
                            .as_ref()
                            .is_some_and(|v| v.to_lowercase().contains(&filter.to_lowercase()))
                    }) {
                        self.scrolling_state.select(Some(i));
                        break;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum QueueActions {
    Delete,
    DeleteAll,
    Play,
    Save,
}
