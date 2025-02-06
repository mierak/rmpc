use anyhow::Result;
use either::Either;
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    text::Line,
};

use super::Pane;
use crate::{
    config::theme::properties::{Property, PropertyKind},
    context::AppContext,
    shared::key_event::KeyEvent,
};

#[derive(Debug)]
pub struct PropertyPane {
    property: &'static [&'static Property<'static, PropertyKind>],
    align: Alignment,
}

impl PropertyPane {
    pub fn new(
        property: &'static [&'static Property<'static, PropertyKind>],
        align: Alignment,
        _context: &AppContext,
    ) -> Self {
        Self { property, align }
    }
}

impl Pane for PropertyPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> Result<()> {
        let song = context.find_current_song_in_queue().map(|(_, song)| song);
        let line = Line::from(self.property.iter().fold(Vec::new(), |mut acc, val| {
            match val.as_span(song, &context.status) {
                Some(Either::Left(span)) => acc.push(span),
                Some(Either::Right(ref mut spans)) => acc.append(spans),
                None => {}
            }
            acc
        }))
        .alignment(self.align);
        frame.render_widget(line, area);
        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _context: &mut AppContext) -> Result<()> {
        Ok(())
    }
}
