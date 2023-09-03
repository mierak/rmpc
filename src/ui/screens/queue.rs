use anyhow::Result;

use crate::{
    mpd::{client::Client, commands::State as MpdState},
    state::PlayListInfoExt,
    ui::{
        modals::Modals,
        widgets::kitty_image::{ImageState, KittyImage},
        DurationExt, Render, SharedUiState,
    },
};
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::{Backend, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, Row, Scrollbar, ScrollbarOrientation, Table, TableState},
    Frame,
};
use tracing::error;

use crate::state::State;

use super::{dirstack::MyState, Screen};

const TABLE_HEADER: &[&str] = &[" Artist", "Title", "Album", "Duration"];

#[derive(Debug, Default)]
pub struct QueueScreen {
    img_state: ImageState,
    scrolling_state: MyState<TableState>,
}

#[async_trait]
impl Screen for QueueScreen {
    fn render<B: Backend>(
        &mut self,
        frame: &mut Frame<B>,
        area: Rect,
        app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> anyhow::Result<()> {
        match (&mut app.album_art, &mut self.img_state.image) {
            (Some(ref mut v), None) => {
                self.img_state.image = Some(crate::state::MyVec(std::mem::take(&mut v.0)));
                self.img_state.needs_transfer = true;
                tracing::debug!(
                    message = "New image received",
                    size = app.album_art.as_ref().map(|a| a.0.len())
                );
            }
            (Some(a), Some(i)) if a.ne(&i) && !a.0.is_empty() => {
                self.img_state.image = Some(crate::state::MyVec(std::mem::take(&mut a.0)));
                self.img_state.needs_transfer = true;
                tracing::debug!(
                    message = "New image received",
                    size = app.album_art.as_ref().map(|a| a.0.len())
                );
            }
            (Some(_), Some(_)) => {} // The image is identical, should be in place already
            (None, None) => {}       // Default img should be in place already
            (None, Some(_)) => {
                // Show default img
                self.img_state.image = None;
                self.img_state.needs_transfer = true;
            }
        }

        let queue_len = app.queue.len().unwrap_or(0);

        let [img_section, queue_section] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                         Constraint::Percentage(35),
                         Constraint::Percentage(65),
            ].as_ref()).split(area) else { return Ok(()) };

        let [table_header_section, mut queue_section] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                         Constraint::Min(2),
                         Constraint::Percentage(100),
            ].as_ref()).split(queue_section) else { return Ok(()) };

        self.scrolling_state.viewport_len(Some(queue_section.height));
        self.scrolling_state.content_len(Some(u16::try_from(queue_len)?));

        let mut rows = Vec::with_capacity(queue_len);
        if let Some(queue) = app.queue.as_ref() {
            for song in &queue.0 {
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

        let header_table = Table::new([])
            .header(Row::new(TABLE_HEADER.to_vec()))
            .block(Block::default().borders(Borders::TOP))
            .widths(&[
                Constraint::Percentage(15),
                Constraint::Percentage(35),
                Constraint::Percentage(35),
                Constraint::Percentage(15),
            ]);

        let table = Table::new(rows)
            .block(Block::default().borders(Borders::TOP))
            .widths(&[
                Constraint::Percentage(15),
                Constraint::Percentage(35),
                Constraint::Percentage(35),
                Constraint::Percentage(15),
            ])
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
        frame.render_stateful_widget(table, queue_section, &mut self.scrolling_state.inner);

        queue_section.y = queue_section.y.saturating_add(1);
        queue_section.height = queue_section.height.saturating_sub(1);
        frame.render_stateful_widget(
            scrollbar,
            queue_section.inner(&ratatui::prelude::Margin {
                vertical: 0,
                horizontal: 0,
            }),
            &mut self.scrolling_state.scrollbar_state,
        );
        frame.render_stateful_widget(
            KittyImage::default().block(Block::default().borders(Borders::TOP)),
            img_section,
            &mut self.img_state,
        );

        Ok(())
    }

    async fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        _app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        self.scrolling_state
            .select(_app.queue.get_by_id(_app.status.songid).map(|v| v.0));
        Ok(())
    }

    async fn handle_key(
        &mut self,
        key: KeyEvent,
        client: &mut Client<'_>,
        app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<Render> {
        match key.code {
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if !app.queue.is_empty_or_none() {
                    for _ in 0..5 {
                        self.scrolling_state.next();
                    }
                }
                return Ok(Render::No);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if !app.queue.is_empty_or_none() {
                    for _ in 0..5 {
                        self.scrolling_state.prev();
                    }
                }
                return Ok(Render::No);
            }
            KeyCode::Char('d') => {
                if let Some(selected_song) = app.queue.get_selected(self.scrolling_state.inner.selected()) {
                    match client.delete_id(selected_song.id).await {
                        Ok(_) => {}
                        Err(e) => error!("{:?}", e),
                    }
                } else {
                    error!("No song selected");
                }
            }
            KeyCode::Char('D') => app.visible_modal = Some(Modals::ConfirmQueueClear),
            KeyCode::Char(' ') if app.status.state == MpdState::Play || app.status.state == MpdState::Pause => {
                client.pause_toggle().await?;
            }
            KeyCode::Enter => {
                if let Some(selected_song) = app.queue.get_selected(self.scrolling_state.inner.selected()) {
                    client.play_id(selected_song.id).await?;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !app.queue.is_empty_or_none() {
                    self.scrolling_state.prev();
                }
                return Ok(Render::No);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !app.queue.is_empty_or_none() {
                    self.scrolling_state.next();
                }
                return Ok(Render::No);
            }
            KeyCode::Char('G') => {
                if !app.queue.is_empty_or_none() {
                    self.scrolling_state.last();
                }
                return Ok(Render::No);
            }
            KeyCode::Char('g') => {
                if !app.queue.is_empty_or_none() {
                    self.scrolling_state.first();
                }
                return Ok(Render::No);
            }
            _ => {}
        };
        Ok(Render::Yes)
    }
}
