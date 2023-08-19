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
    widgets::{Block, Borders, LineGauge, Paragraph, Row, Table, Wrap},
    Frame,
};
use tracing::error;

use crate::{mpd::errors::MpdError, state::State};

use super::Screen;

#[derive(Debug, Default)]
pub struct QueueScreen {
    frame_counter: FrameCounter,
    img_state: ImageState,
}

#[async_trait]
impl Screen for QueueScreen {
    fn render(
        &mut self,
        frame: &mut Frame<CrosstermBackend<Stdout>>,
        area: Rect,
        app: &crate::state::State,
        _shared: &SharedUiState,
    ) -> anyhow::Result<()> {
        if app.album_art.ne(&self.img_state.image) {
            // TODO remove the clone
            // drain? take?
            self.img_state.image = app.album_art.clone();
            self.img_state.needs_transfer = true;
            tracing::debug!(
                message = "New image received",
                size = app.album_art.as_ref().map(|a| a.0.len())
            );
        }

        let mut rows = Vec::with_capacity(app.queue.as_ref().map_or(0, |v| v.0.len()));
        if let Some(queue) = app.queue.as_ref() {
            for song in queue.0.iter() {
                let mut row = Row::new(vec![
                    song.artist.as_ref().map_or("-", |v| v).to_owned(),
                    song.title.as_ref().map_or("-", |v| v).to_owned(),
                    song.album.as_ref().map_or("-", |v| v).to_owned(),
                    song.duration.as_ref().map_or("-".to_string(), |v| v.to_string()),
                ]);
                if app.status.songid.as_ref().is_some_and(|v| *v == song.id) {
                    row = row.style(Style::default().fg(Color::Blue));
                }
                if song.selected {
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

        let table = Table::new(rows).widths(&[
            Constraint::Percentage(15),
            Constraint::Percentage(35),
            Constraint::Percentage(35),
            Constraint::Percentage(15),
        ]);

        let volume = LineGauge::default()
            .block(
                Block::default()
                    .title("Volume")
                    .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT),
            )
            .gauge_style(
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Black)
                    .add_modifier(Modifier::ITALIC),
            )
            .ratio((*app.status.volume.value() as f32 / 100.0).into());
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

        let [top, queue_section, logs] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(2),
                    Constraint::Percentage(50),
                    Constraint::Percentage(70),
                ]
                .as_ref(),
            ).split(area) else {
                return Ok(())
            };
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
        frame.render_widget(table, queue_section);
        frame.render_widget(logs_wg, logs);
        frame.render_stateful_widget(
            KittyImage::default().block(Block::default().borders(Borders::TOP)),
            img_section,
            &mut self.img_state,
        );
        self.frame_counter.increment();

        Ok(())
    }

    async fn handle_key(
        &mut self,
        key: KeyEvent,
        client: &mut Client<'_>,
        app: &mut State,
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
                // TODO select another song after delete
                if let Some(selected_song) = app.queue.as_ref().and_then(|q| q.0.iter().find(|s| s.selected)) {
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
                if let Some(queue) = app.queue.as_mut() {
                    if let Some(song) = queue.0.iter().find(|s| s.selected) {
                        client.play_id(song.id).await?;
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(queue) = app.queue.as_mut() {
                    if let Some((idx, song)) = queue.0.iter_mut().enumerate().find(|s| s.1.selected) {
                        song.selected = false;
                        if idx > 0 {
                            queue.0[idx - 1].selected = true;
                        } else {
                            queue.0.last_mut().unwrap().selected = true;
                        }
                    } else if !queue.0.is_empty() {
                        queue.0.last_mut().unwrap().selected = true;
                    }
                }
                return Ok(Render::NoSkip);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(queue) = app.queue.as_mut() {
                    if let Some((idx, song)) = queue.0.iter_mut().enumerate().find(|s| s.1.selected) {
                        song.selected = false;
                        if idx < queue.0.len() - 1 {
                            queue.0[idx + 1].selected = true;
                        } else {
                            queue.0.first_mut().unwrap().selected = true;
                        }
                    } else if !queue.0.is_empty() {
                        queue.0.first_mut().unwrap().selected = true;
                    }
                }
                return Ok(Render::NoSkip);
            }
            _ => {}
        };
        Ok(Render::Skip)
    }
}
