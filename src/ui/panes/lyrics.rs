use std::{fs, path::PathBuf};

use anyhow::{bail, Result};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Text,
    Frame,
};

use crate::{
    context::AppContext,
    mpd::mpd_client::MpdClient,
    shared::{key_event::KeyEvent, lrc::Lrc, macros::status_error},
    ui::UiEvent,
};

use super::Pane;

#[derive(Debug)]
pub struct LyricsPane {
    current_lyrics: Option<Lrc>,
    initialized: bool,
}

impl LyricsPane {
    pub fn new(_context: &AppContext) -> Self {
        Self {
            current_lyrics: None,
            initialized: false,
        }
    }

    fn find_lrc(context: &AppContext) -> Result<Option<Lrc>> {
        let Some((_, song)) = context.find_current_song_in_queue() else {
            return Ok(None);
        };

        let Some(lyrics_dir) = context.config.lyrics_dir else {
            return Ok(None);
        };

        let mut path: PathBuf = PathBuf::from(lyrics_dir);
        path.push(&song.file);
        let Some(stem) = path.file_stem().map(|stem| format!("{}.lrc", stem.to_string_lossy())) else {
            bail!("No file stem for lyrics path: {path:?}");
        };

        path.pop();
        path.push(stem);
        match fs::read_to_string(&path) {
            Ok(lrc) => Ok(Some(lrc.parse()?)),
            Err(err) if matches!(err.kind(), std::io::ErrorKind::NotFound) => {
                log::trace!(path:?; "LRC file not found");
                Ok(None)
            }
            Err(err) => Err(err.into()),
        }
    }
}

impl Pane for LyricsPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> Result<()> {
        let Some(lrc) = &self.current_lyrics else { return Ok(()) };

        let elapsed = context.status.elapsed;
        let Some((current_line_idx, _)) = lrc
            .lines
            .iter()
            .enumerate()
            .filter(|line| line.1.time > elapsed)
            .min_by(|a, b| a.1.time.abs_diff(elapsed).cmp(&b.1.time.abs_diff(elapsed)))
        else {
            return Ok(());
        };

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

        Ok(())
    }

    fn before_show(&mut self, _client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        if !self.initialized {
            match Self::find_lrc(context) {
                Ok(lrc) => {
                    self.current_lyrics = lrc;
                }
                Err(err) => {
                    status_error!("Failed to load lyrics file: '{err}'");
                }
            }
            self.initialized = true;
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, _client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        if let UiEvent::Player = event {
            match Self::find_lrc(context) {
                Ok(lrc) => {
                    self.current_lyrics = lrc;
                }
                Err(err) => {
                    status_error!("Failed to load lyrics file: '{err}'");
                }
            }
        }
        Ok(())
    }

    fn handle_action(
        &mut self,
        _event: &mut KeyEvent,
        _client: &mut impl MpdClient,
        _context: &AppContext,
    ) -> Result<()> {
        Ok(())
    }
}
