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
    ctx::Ctx,
    shared::key_event::KeyEvent,
    ui::widgets::scrolling_line::ScrollingLine,
};

#[derive(Debug)]
pub struct PropertyPane<'content> {
    content: &'content Vec<Property<PropertyKind>>,
    align: Alignment,
    scroll_speed: u64,
}

impl<'content> PropertyPane<'content> {
    pub fn new(
        content: &'content Vec<Property<PropertyKind>>,
        align: Alignment,
        scroll_speed: u64,
        _ctx: &Ctx,
    ) -> Self {
        Self { content, align, scroll_speed }
    }
}

impl Pane for PropertyPane<'_> {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        let song = ctx.find_current_song_in_queue().map(|(_, song)| song);
        let song_stickers = song.and_then(|s| ctx.stickers.get(&s.file));

        let line = Line::from(self.content.iter().fold(Vec::new(), |mut acc, val| {
            match val.as_span(
                song,
                song_stickers,
                ctx,
                &ctx.config.theme.format_tag_separator,
                ctx.config.theme.multiple_tag_resolution_strategy,
            ) {
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
            .progress(ctx.status.elapsed)
            .build();
        frame.render_widget(scrolling_line, area);

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut KeyEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}
