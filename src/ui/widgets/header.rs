use either::Either;
use ratatui::{
    prelude::{Constraint, Layout},
    style::Style,
    text::Line,
    widgets::{Block, Widget},
};

use crate::{
    config::{
        Config,
        theme::properties::{Property, PropertyKind},
    },
    ctx::Ctx,
    mpd::commands::Song,
};

pub struct Header<'a> {
    context: &'a Ctx,
}

impl Widget for Header<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let config = &self.context.config;

        if let Some(header_bg_color) = config.theme.header_background_color {
            Block::default().style(Style::default().bg(header_bg_color)).render(area, buf);
        }

        let row_count = config.theme.header.rows.len();

        let layouts = Layout::vertical((0..row_count).map(|_| Constraint::Length(1))).split(area);
        let song = self.context.find_current_song_in_queue().map(|(_, song)| song);
        for row in 0..row_count {
            let [left, center, right] = *Layout::horizontal([
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ])
            .split(layouts[row]) else {
                return;
            };
            let template = PropertyTemplates(&config.theme.header.rows[row].left);
            let widget = template.format(song, self.context, &self.context.config).left_aligned();
            widget.render(left, buf);

            let template = PropertyTemplates(&config.theme.header.rows[row].center);
            let widget = template.format(song, self.context, &self.context.config).centered();
            widget.render(center, buf);

            let template = PropertyTemplates(&config.theme.header.rows[row].right);
            let widget = template.format(song, self.context, &self.context.config).right_aligned();
            widget.render(right, buf);
        }
    }
}

struct PropertyTemplates<'a>(&'a [Property<PropertyKind>]);
impl<'a> PropertyTemplates<'a> {
    fn format(&'a self, song: Option<&'a Song>, context: &'a Ctx, config: &Config) -> Line<'a> {
        Line::from(self.0.iter().fold(Vec::new(), |mut acc, val| {
            match val.as_span(
                song,
                context,
                &config.theme.format_tag_separator,
                config.theme.multiple_tag_resolution_strategy,
            ) {
                Some(Either::Left(span)) => acc.push(span),
                Some(Either::Right(ref mut spans)) => acc.append(spans),
                None => {}
            }
            acc
        }))
    }
}

impl<'a> Header<'a> {
    pub fn new(context: &'a Ctx) -> Self {
        Self { context }
    }
}
