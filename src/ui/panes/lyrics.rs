use anyhow::Result;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Text,
};

use super::Pane;
use crate::{
    context::Ctx,
    shared::{
        ext::duration::DurationExt,
        key_event::KeyEvent,
        lrc::Lrc,
        macros::status_error,
        mpd_query::run_status_update,
    },
    ui::UiEvent,
};

#[derive(Debug)]
pub struct LyricsPane {
    current_lyrics: Option<Lrc>,
    initialized: bool,
    last_requested_line_idx: usize,
}

impl LyricsPane {
    pub fn new(_context: &Ctx) -> Self {
        Self { current_lyrics: None, initialized: false, last_requested_line_idx: 0 }
    }
}

impl Pane for LyricsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &Ctx) -> Result<()> {
        let Some(lrc) = &self.current_lyrics else { return Ok(()) };

        let elapsed = context.status.elapsed;
        let (current_line_idx, first_line_reached) = lrc
            .lines
            .iter()
            .enumerate()
            .filter(|line| elapsed >= line.1.time)
            .min_by(|a, b| a.1.time.abs_diff(elapsed).cmp(&b.1.time.abs_diff(elapsed)))
            .map_or((0, false), |result| (result.0, true));

        let rows = area.height;
        let areas = Layout::vertical((0..rows).map(|_| Constraint::Length(1))).split(area);
        let middle_row = rows / 2;

        let default_style =
            Style::default().fg(context.config.theme.text_color.unwrap_or_default());

        let middle_style = if first_line_reached {
            context.config.theme.highlighted_item_style
        } else {
            default_style
        };

        let timestamp = context.config.theme.lyrics.timestamp;

        let mut current_area = middle_row as usize;
        let Some(current_line) = lrc.lines.get(current_line_idx) else {
            return Ok(());
        };
        let formatted_line = if timestamp && !current_line.content.is_empty() {
            &format!("[{}] {}", current_line.time.to_string(), current_line.content)
        } else {
            &current_line.content
        };
        for l in textwrap::wrap(formatted_line, area.width as usize) {
            let Some(area) = areas.get(current_area) else {
                break;
            };
            let text = Text::from(l).centered().style(middle_style);
            frame.render_widget(text, *area);
            current_area += 1;
        }

        let mut before_lyrics_cursor = current_line_idx;
        let mut before_area_cursor = middle_row as usize;
        while before_lyrics_cursor > 0 && before_area_cursor > 0 {
            before_lyrics_cursor -= 1;
            let Some(line) = lrc.lines.get(before_lyrics_cursor) else {
                break;
            };
            let formatted_line = if timestamp && !line.content.is_empty() {
                &format!("[{}] {}", line.time.to_string(), line.content)
            } else {
                &line.content
            };
            for l in textwrap::wrap(formatted_line, area.width as usize).iter().rev() {
                if before_area_cursor == 0 {
                    break;
                }
                let Some(area) = areas.get(before_area_cursor - 1) else {
                    break;
                };
                let text = Text::from(l.as_ref()).centered().style(default_style);

                frame.render_widget(text, *area);
                before_area_cursor -= 1;
            }
        }
        let mut after_lyrics_cursor = current_line_idx;
        let mut after_area_cursor = current_area.saturating_sub(1);

        while !areas.is_empty()
            && after_lyrics_cursor < lrc.lines.len() - 1
            && after_area_cursor < areas.len() - 1
        {
            after_lyrics_cursor += 1;
            let Some(line) = lrc.lines.get(after_lyrics_cursor) else {
                break;
            };
            let formatted_line = if timestamp && !line.content.is_empty() {
                &format!("[{}] {}", line.time.to_string(), line.content)
            } else {
                &line.content
            };
            for l in textwrap::wrap(formatted_line, area.width as usize) {
                let Some(area) = areas.get(after_area_cursor + 1) else {
                    break;
                };
                let text = Text::from(l).centered().style(default_style);
                frame.render_widget(text, *area);
                after_area_cursor += 1;
            }
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

    fn before_show(&mut self, context: &Ctx) -> Result<()> {
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

    fn on_event(&mut self, event: &mut UiEvent, _is_visible: bool, context: &Ctx) -> Result<()> {
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

    fn handle_action(&mut self, _event: &mut KeyEvent, _context: &mut Ctx) -> Result<()> {
        Ok(())
    }
}
