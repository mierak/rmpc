use either::Either;
use ratatui::{
    prelude::{Constraint, Layout},
    style::Style,
    text::Line,
    widgets::{Block, Widget},
};

use crate::{
    config::{
        theme::properties::{Property, PropertyKind},
        Config,
    },
    mpd::commands::{Song, Status},
};

pub struct Header<'a> {
    config: &'a Config,
    status: &'a Status,
    song: Option<&'a Song>,
}

impl Widget for Header<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let config = self.config;

        if let Some(header_bg_color) = config.theme.header_background_color {
            Block::default()
                .style(Style::default().bg(header_bg_color))
                .render(area, buf);
        }

        let row_count = config.theme.header.rows.len();

        let layouts = Layout::vertical((0..row_count).map(|_| Constraint::Length(1))).split(area);
        for row in 0..row_count {
            let [left, center, right] = *Layout::horizontal([
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ])
            .split(layouts[row]) else {
                return;
            };
            let template = PropertyTemplates(config.theme.header.rows[row].left);
            let widget = template.format(self.song, self.status).left_aligned();
            widget.render(left, buf);

            let template = PropertyTemplates(config.theme.header.rows[row].center);
            let widget = template.format(self.song, self.status).centered();
            widget.render(center, buf);

            let template = PropertyTemplates(config.theme.header.rows[row].right);
            let widget = template.format(self.song, self.status).right_aligned();
            widget.render(right, buf);
        }
    }
}

struct PropertyTemplates<'a>(&'a [&'a Property<'static, PropertyKind>]);
impl<'a> PropertyTemplates<'a> {
    fn format(&'a self, song: Option<&'a Song>, status: &'a Status) -> Line<'a> {
        Line::from(self.0.iter().fold(Vec::new(), |mut acc, val| {
            match val.as_span(song, status) {
                Some(Either::Left(span)) => acc.push(span),
                Some(Either::Right(ref mut spans)) => acc.append(spans),
                None => {}
            }
            acc
        }))
    }
}

impl<'a> Header<'a> {
    pub fn new(config: &'a Config, status: &'a Status, song: Option<&'a Song>) -> Self {
        Self { config, status, song }
    }
}
