use anyhow::Result;
use either::Either;
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::Line,
    widgets::Block,
};

use super::Pane;
use crate::{
    config::theme::properties::{Property, PropertyKind},
    context::AppContext,
    shared::key_event::KeyEvent,
    ui::widgets::scrolling_line::ScrollingLine,
};

#[derive(Debug)]
pub struct PropertyPane<'content> {
    content: &'content Vec<Property<PropertyKind>>,
    align: Alignment,
    scroll_speed: u64,
    background_color: Option<Color>,
}

impl<'content> PropertyPane<'content> {
    pub fn new(
        content: &'content Vec<Property<PropertyKind>>,
        align: Alignment,
        scroll_speed: u64,
        background_color: Option<Color>,
        _context: &AppContext,
    ) -> Self {
        Self { content, align, scroll_speed, background_color }
    }
}

impl Pane for PropertyPane<'_> {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> Result<()> {
        if let Some(bg_color) = self.background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), area);
        }
        let song = context.find_current_song_in_queue().map(|(_, song)| song);

        let line = Line::from(self.content.iter().fold(Vec::new(), |mut acc, val| {
            match val.as_span(song, &context.status) {
                Some(Either::Left(span)) => acc.push(span),
                Some(Either::Right(ref mut spans)) => acc.append(spans),
                None => {}
            }
            acc
        }));

        let scrolling_line = ScrollingLine::builder()
            .scroll_speed(self.scroll_speed)
            .align(self.align)
            .line(line)
            .progress(context.status.elapsed)
            .build();
        frame.render_widget(scrolling_line, area);

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _context: &mut AppContext) -> Result<()> {
        Ok(())
    }
}
