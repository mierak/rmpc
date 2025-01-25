use anyhow::Result;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Text,
};

use super::Pane;
use crate::{
    context::AppContext,
    shared::{key_event::KeyEvent, lrc::Lrc, macros::status_error, mpd_query::run_status_update},
    ui::UiEvent,
};

#[derive(Debug)]
pub struct LyricsPane {
    current_lyrics: Option<Lrc>,
    initialized: bool,
    last_requested_line_idx: usize,
}

impl LyricsPane {
    pub fn new(_context: &AppContext) -> Self {
        Self { current_lyrics: None, initialized: false, last_requested_line_idx: 0 }
    }
}

impl Pane for LyricsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> Result<()> {
        let Some(lrc) = &self.current_lyrics else { return Ok(()) };

        let elapsed = context.status.elapsed;
        let current_line_idx = lrc
            .lines
            .iter()
            .enumerate()
            .filter(|line| elapsed >= line.1.time)
            .min_by(|a, b| a.1.time.abs_diff(elapsed).cmp(&b.1.time.abs_diff(elapsed)))
            .map(|result| result.0)
            .unwrap_or_default();

        let rows = area.height;
        let areas = Layout::vertical((0..rows).map(|_| Constraint::Length(1))).split(area);
        let middle_row = rows / 2;

        for i in 0..rows {
            let i = i as usize;
            let Some(idx) = (current_line_idx + i).checked_sub(middle_row as usize) else {
                continue;
            };
            let Some(line) = lrc.lines.get(idx) else {
                continue;
            };

            let darken = (middle_row as usize).abs_diff(i) > 0;

            let p = Text::from(line.content.clone()).centered().style(if darken {
                Style::default().fg(context.config.theme.text_color.unwrap_or_default())
            } else {
                context.config.theme.highlighted_item_style
            });

            frame.render_widget(p, areas[i]);
        }

        // Try to schedule the next line to be displayed on time
        if self.last_requested_line_idx != current_line_idx + 1 {
            if let Some(line) = lrc.lines.get(current_line_idx + 1) {
                self.last_requested_line_idx = current_line_idx + 1;
                context
                    .scheduler
                    .schedule(line.time.saturating_sub(context.status.elapsed), run_status_update);
            }
        }

        Ok(())
    }

    fn before_show(&mut self, context: &AppContext) -> Result<()> {
        if !self.initialized {
            match context.find_lrc() {
                Ok(lrc) => {
                    self.current_lyrics = lrc;
                }
                Err(err) => {
                    status_error!("Failed to load lyrics file: '{err}'");
                    self.current_lyrics = None;
                }
            }
            self.last_requested_line_idx = 0;
            self.initialized = true;
        }

        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut UiEvent,
        _is_visible: bool,
        context: &AppContext,
    ) -> Result<()> {
        match event {
            UiEvent::SongChanged | UiEvent::Reconnected => {
                match context.find_lrc() {
                    Ok(lrc) => {
                        self.current_lyrics = lrc;
                        context.render()?;
                    }
                    Err(err) => {
                        self.current_lyrics = None;
                        status_error!("Failed to load lyrics file: '{err}'");
                    }
                }
                self.last_requested_line_idx = 0;
            }
            UiEvent::LyricsIndexed if self.current_lyrics.is_none() => {
                match context.find_lrc() {
                    Ok(lrc) => {
                        self.current_lyrics = lrc;
                        context.render()?;
                    }
                    Err(err) => {
                        self.current_lyrics = None;
                        status_error!("Failed to load lyrics file: '{err}'");
                    }
                }
                self.last_requested_line_idx = 0;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _context: &mut AppContext) -> Result<()> {
        Ok(())
    }
}
