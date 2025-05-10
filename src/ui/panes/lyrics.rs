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

        let mut current_area_cursor = middle_row;
        let current_line_wrapped =
            textwrap::wrap(&lrc.lines[current_line_idx].content, area.width as usize);
        for l in current_line_wrapped {
            let p = Text::from(l).centered().style(context.config.theme.highlighted_item_style);
            frame.render_widget(p, areas[current_area_cursor as usize]);
            current_area_cursor += 1;
        }

        let mut before_lyrics_cursor = current_line_idx;
        let mut before_area_cursor = middle_row as usize;
        while before_lyrics_cursor > 0 && before_area_cursor > 0 {
            before_lyrics_cursor -= 1;
            let Some(line) = lrc.lines.get(before_lyrics_cursor) else {
                break;
            };
            let wrapped_line = textwrap::wrap(&line.content, area.width as usize);
            for l in wrapped_line.iter().rev() {
                let p = Text::from(l.clone()).centered().style(
                    Style::default().fg(context.config.theme.text_color.unwrap_or_default()),
                );
                if before_area_cursor == 0 {
                    break;
                }
                before_area_cursor -= 1;
                frame.render_widget(p, areas[before_area_cursor]);
            }
        }
        let mut after_lyrics_cursor = current_line_idx;
        let mut after_area_cursor = (current_area_cursor - 1) as usize;

        while after_lyrics_cursor < lrc.lines.len() - 1 && after_area_cursor < areas.len() - 1 {
            after_lyrics_cursor += 1;
            let Some(line) = lrc.lines.get(after_lyrics_cursor) else {
                break;
            };
            let wrapped_line = textwrap::wrap(&line.content, area.width as usize);
            for l in wrapped_line {
                let p = Text::from(l).centered().style(
                    Style::default().fg(context.config.theme.text_color.unwrap_or_default()),
                );
                after_area_cursor += 1;
                if after_area_cursor >= areas.len() {
                    break;
                }
                frame.render_widget(p, areas[after_area_cursor]);
            }
        }

        // let cum_prev_line_wrap_count = if current_line_idx > 0 {
        //     lrc.lines[..current_line_idx]
        //         .iter()
        //         .map(|line| textwrap::wrap(&line.content, area.width as usize).len()
        // - 1)         .sum::<usize>()
        // } else {
        //     0
        // };
        //
        // while area_idx < (rows as usize) {
        //     if (current_line_idx + area_idx + cum_prev_line_wrap_count)
        //         .checked_sub(middle_row as usize)
        //         .is_none()
        //     {
        //         area_idx += 1;
        //         continue;
        //     }
        //
        //     let Some(idx) = (current_line_idx +
        // lyrics_fetch_idx).checked_sub(middle_row as usize)     else {
        //         lyrics_fetch_idx += 1;
        //         continue;
        //     };
        //
        //     let Some(line) = lrc.lines.get(idx) else {
        //         lyrics_fetch_idx += 1;
        //         continue;
        //     };
        //
        //     let wrapped_line = textwrap::wrap(&line.content, area.width as usize);
        //
        //     let darken = (middle_row as usize).abs_diff(area_idx) > 0 ||
        // !first_line_reached;
        //
        //     for l in wrapped_line {
        //         let p = Text::from(l).centered().style(if darken {
        //
        // Style::default().fg(context.config.theme.text_color.unwrap_or_default())
        //         } else {
        //             context.config.theme.highlighted_item_style
        //         });
        //         frame.render_widget(p, areas[area_idx]);
        //         area_idx += 1;
        //         if area_idx >= (rows as usize) {
        //             break;
        //         }
        //     }
        //     lyrics_fetch_idx += 1;
        // }

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
