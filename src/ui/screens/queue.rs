use anyhow::Result;

use crate::{
    mpd::{client::Client, commands::State as MpdState},
    ui::{
        widgets::{
            kitty_image::{ImageState, KittyImage},
            scrollbar::{Scrollbar, ScrollbarState},
        },
        DurationExt, Render, SharedUiState,
    },
};
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    prelude::{Backend, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, Row, ScrollDirection, ScrollbarOrientation, Table},
    Frame,
};
use tracing::error;

use crate::{mpd::errors::MpdError, state::State};

use super::Screen;

#[derive(Debug, Default)]
pub struct QueueScreen {
    img_state: ImageState,
    scrollbar: ScrollbarState,
    should_center: bool,
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

        let [img_section, queue_section] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                         Constraint::Percentage(35),
                         Constraint::Percentage(65),
            ].as_ref()).split(area) else { return Ok(()) };

        let [table_header_section, queue_section] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                         Constraint::Min(3),
                         Constraint::Percentage(100),
            ].as_ref()).split(queue_section) else { return Ok(()) };

        let [queue_section, scrollbar_section] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                         Constraint::Percentage(100),
                         Constraint::Min(1),
            ].as_ref()).split(queue_section) else { return Ok(()) };

        self.scrollbar
            .content_length(app.queue.as_ref().map_or(0, |v| v.0.len()) as u16);
        self.scrollbar.viewport_content_length(queue_section.height);
        if self.should_center {
            self.should_center = false;
            self.scrollbar.center_on(self.scrollbar.get_position());
        }

        let mut rows = Vec::with_capacity(app.queue.as_ref().map_or(0, |v| v.0.len()));
        if let Some(queue) = app.queue.as_ref() {
            for (idx, song) in queue.0.iter().enumerate() {
                let mut row = Row::new(vec![
                    song.artist.as_ref().map_or("-".to_owned(), |v| format!(" {v}")),
                    song.title.as_ref().map_or("-", |v| v).to_owned(),
                    song.album.as_ref().map_or("-", |v| v).to_owned(),
                    song.duration.as_ref().map_or("-".to_string(), |v| v.to_string()),
                ]);
                if app.status.songid.as_ref().is_some_and(|v| *v == song.id) {
                    row = row.style(Style::default().fg(Color::Blue));
                }
                if idx as u16 == self.scrollbar.get_position() {
                    row = row.style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
                }
                rows.push(row)
            }
        }

        let header_table = Table::new([])
            .header(Row::new(vec!["  Artist", "Title", "Album", "Duration"]))
            .block(Block::default().borders(Borders::TOP | Borders::BOTTOM))
            .widths(&[
                Constraint::Percentage(15),
                Constraint::Percentage(35),
                Constraint::Percentage(35),
                Constraint::Percentage(15),
            ]);

        let table = Table::new(rows[self.scrollbar.get_range_usize()].to_vec()).widths(&[
            Constraint::Percentage(15),
            Constraint::Percentage(35),
            Constraint::Percentage(35),
            Constraint::Percentage(15),
        ]);

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .track_symbol("│")
            .end_symbol(Some("↓"))
            .track_style(Style::default().fg(Color::White).bg(Color::Black))
            .begin_style(Style::default().fg(Color::White).bg(Color::Black))
            .end_style(Style::default().fg(Color::White).bg(Color::Black))
            .thumb_style(Style::default().fg(Color::Blue));

        frame.render_widget(header_table, table_header_section);
        frame.render_widget(
            table,
            queue_section.inner(&ratatui::prelude::Margin {
                vertical: 0,
                horizontal: 1,
            }),
        );
        frame.render_stateful_widget(scrollbar, scrollbar_section, &mut self.scrollbar.inner);
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
        if self.scrollbar.get_position() == 0 {
            if let Some(queue) = _app.queue.as_ref() {
                for (idx, song) in queue.0.iter().enumerate() {
                    if _app.status.songid.as_ref().is_some_and(|v| *v == song.id) {
                        self.should_center = true;
                        self.scrollbar.center_on(idx as u16);
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_key(
        &mut self,
        key: KeyEvent,
        client: &mut Client<'_>,
        app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<Render, MpdError> {
        match key.code {
            KeyCode::Char('d') => {
                if let Some(Some(selected_song)) = app
                    .queue
                    .as_ref()
                    .map(|v| v.0.get(self.scrollbar.get_position() as usize))
                {
                    match client.delete_id(selected_song.id).await {
                        Ok(_) => {}
                        Err(e) => error!("{:?}", e),
                    }
                } else {
                    error!("No song selected");
                }
            }
            KeyCode::Char(' ') if app.status.state == MpdState::Play || app.status.state == MpdState::Pause => {
                client.pause_toggle().await?;
            }
            KeyCode::Enter => {
                if let Some(Some(selected_song)) = app
                    .queue
                    .as_ref()
                    .map(|v| v.0.get(self.scrollbar.get_position() as usize))
                {
                    client.play_id(selected_song.id).await?;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if app.queue.as_ref().is_some_and(|q| !q.0.is_empty()) {
                    self.scrollbar.scroll(ScrollDirection::Backward);
                }
                return Ok(Render::NoSkip);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if app.queue.as_ref().is_some_and(|q| !q.0.is_empty()) {
                    self.scrollbar.scroll(ScrollDirection::Forward);
                }
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('G') => {
                if app.queue.as_ref().is_some_and(|q| !q.0.is_empty()) {
                    self.scrollbar.last();
                }
                return Ok(Render::NoSkip);
            }
            KeyCode::Char('g') => {
                if app.queue.as_ref().is_some_and(|q| !q.0.is_empty()) {
                    self.scrollbar.first();
                }
                return Ok(Render::NoSkip);
            }
            _ => {}
        };
        Ok(Render::Skip)
    }
}
