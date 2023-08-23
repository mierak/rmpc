use anyhow::Result;
use std::io::Stdout;

use crate::{
    mpd::{
        client::Client,
        commands::{volume::Bound, State as MpdState},
    },
    ui::{
        widgets::{
            frame_counter::FrameCounter,
            kitty_image::{ImageState, KittyImage},
            scrollbar::{Scrollbar, ScrollbarState},
        },
        Render, SharedUiState,
    },
};
use ansi_to_tui::IntoText;
use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::{Alignment, Constraint, CrosstermBackend, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, ScrollDirection, ScrollbarOrientation, Table, Wrap},
    Frame,
};
use tracing::error;

use crate::{mpd::errors::MpdError, state::State};

use super::Screen;

#[derive(Debug, Default)]
pub struct QueueScreen {
    frame_counter: FrameCounter,
    img_state: ImageState,
    scrollbar: ScrollbarState,
    should_center: bool,
}

#[async_trait]
impl Screen for QueueScreen {
    fn render(
        &mut self,
        frame: &mut Frame<CrosstermBackend<Stdout>>,
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

        let [top, queue_section, logs] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(2),
                    Constraint::Percentage(50),
                    Constraint::Percentage(70),
                ]
                .as_ref(),
            ).split(area) else { return Ok(()) };

        let [img_section, queue_section] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                         Constraint::Percentage(25),
                         Constraint::Percentage(75),
            ].as_ref()).split(queue_section) else { return Ok(()) };

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

        let [left, left2, right3, right2, right] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20),
                    Constraint::Percentage(20)
                ].as_ref(),
            ) .split(top) else { return Ok(())};

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
                    song.duration.as_ref().map_or("-".to_string(), |v| {
                        let secs = v.as_secs();
                        let min = secs / 60;
                        format!("{}:{:0>2}", min, secs - min * 60)
                    }),
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
            .header(Row::new(vec!["Artist", "Tille", "Album", "Duration"]))
            .block(Block::default().borders(Borders::TOP | Borders::BOTTOM))
            .widths(&[
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
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

        let volume = crate::ui::widgets::volume::Volume::default()
            .value(*app.status.volume.value())
            .block(
                Block::default()
                    .title("Volume")
                    .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT),
            )
            .style(Style::default().fg(Color::Blue));

        let repeat = Paragraph::new(if app.status.repeat { "On" } else { "Off" }).block(
            Block::default()
                .title("Repeat")
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT),
        );
        let random = Paragraph::new(if app.status.random { "On" } else { "Off" }).block(
            Block::default()
                .title("Random")
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT),
        );
        let status = Paragraph::new(format!("{}", app.status.state)).block(
            Block::default()
                .title("Status")
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT),
        );

        let logs_wg = Paragraph::new(
            app.logs
                .0
                .iter()
                .flat_map(|l| l.into_text().unwrap().lines)
                .collect::<Vec<Line>>(),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Gray))
                .title(Span::styled(
                    format!("Logs: {}", app.logs.0.len()),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
        )
        .alignment(Alignment::Left)
        .scroll(((app.logs.0.len() as u16).max(logs.height) - logs.height, 0))
        .wrap(Wrap { trim: true });

        frame.render_widget(repeat, left2);
        frame.render_widget(random, right2);
        frame.render_widget(status, right3);
        frame.render_widget(volume, right);
        frame.render_widget(&self.frame_counter, left);
        frame.render_widget(header_table, table_header_section);
        frame.render_widget(
            table,
            queue_section.inner(&ratatui::prelude::Margin {
                vertical: 0,
                horizontal: 1,
            }),
        );
        frame.render_stateful_widget(scrollbar, scrollbar_section, &mut self.scrollbar.inner);
        frame.render_widget(logs_wg, logs);
        frame.render_stateful_widget(
            KittyImage::default().block(Block::default().borders(Borders::TOP)),
            img_section,
            &mut self.img_state,
        );
        self.frame_counter.increment();

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
            // these two are here only to induce panic for testing
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => client.next().await?,
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => client.prev().await?,

            KeyCode::Char('n') if app.status.state == MpdState::Play => client.next().await?,
            KeyCode::Char('p') if app.status.state == MpdState::Play => client.prev().await?,
            KeyCode::Char('s') if app.status.state == MpdState::Play => client.stop().await?,
            KeyCode::Char('z') => client.repeat(!app.status.repeat).await?,
            KeyCode::Char('x') => client.random(!app.status.random).await?,
            KeyCode::Char('f') if app.status.state == MpdState::Play => client.seek_curr_forwards(5).await?,
            KeyCode::Char('b') if app.status.state == MpdState::Play => client.seek_curr_backwards(5).await?,
            KeyCode::Char(',') => client.set_volume(app.status.volume.dec()).await?,
            KeyCode::Char('.') => client.set_volume(app.status.volume.inc()).await?,
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
