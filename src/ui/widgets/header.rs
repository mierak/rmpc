use either::Either;
use ratatui::{
    prelude::{Constraint, Layout},
    style::Style,
    text::Line,
    widgets::{Block, Widget},
};
use strum::{IntoEnumIterator, VariantNames};

use crate::{
    config::{
        ui::properties::{Property, PropertyKind},
        Config,
    },
    mpd::commands::{Song, Status},
};

use super::app_tabs::AppTabs;

pub struct Header<'a, T>
where
    T: PartialEq + IntoEnumIterator + VariantNames,
{
    config: &'a Config,
    status: &'a Status,
    active_tab: T,
    song: Option<&'a Song>,
}

impl<T> Widget for Header<'_, T>
where
    T: PartialEq + IntoEnumIterator + VariantNames,
{
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let config = self.config;

        if let Some(header_bg_color) = config.ui.header_background_color {
            Block::default()
                .style(Style::default().bg(header_bg_color))
                .render(area, buf);
        }

        let row_count = config.ui.header.rows.len();

        let [header, tabs] = *Layout::vertical([
            Constraint::Length(row_count as u16),
            Constraint::Length(if config.ui.draw_borders { 3 } else { 1 }),
        ])
        .split(area) else {
            return;
        };

        let layouts = Layout::vertical((0..row_count).map(|_| Constraint::Length(1))).split(header);
        for row in 0..row_count {
            let [left, center, right] = *Layout::horizontal([
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ])
            .split(layouts[row]) else {
                return;
            };
            let template = PropertyTemplates(config.ui.header.rows[row].left);
            let widget = template.format(self.song, self.status).left_aligned();
            widget.render(left, buf);

            let template = PropertyTemplates(config.ui.header.rows[row].center);
            let widget = template.format(self.song, self.status).centered();
            widget.render(center, buf);

            let template = PropertyTemplates(config.ui.header.rows[row].right);
            let widget = template.format(self.song, self.status).right_aligned();
            widget.render(right, buf);
        }

        let app_tabs = AppTabs::new(self.active_tab, config);
        app_tabs.render(tabs, buf);
    }
}

struct PropertyTemplates<'a>(&'a [&'a Property<'static, PropertyKind>]);
impl<'a> PropertyTemplates<'a> {
    fn format(&'a self, song: Option<&'a Song>, status: &'a Status) -> Line<'a> {
        Line::from(self.0.iter().fold(Vec::new(), |mut acc, val| {
            match val.as_span(song, status) {
                Either::Left(span) => acc.push(span),
                Either::Right(ref mut spans) => acc.append(spans),
            }
            acc
        }))
    }
}

impl<'a, T> Header<'a, T>
where
    T: PartialEq + IntoEnumIterator + VariantNames,
{
    pub fn new(config: &'a Config, active_tab: T, status: &'a Status, song: Option<&'a Song>) -> Self {
        Self {
            config,
            status,
            active_tab,
            song,
        }
    }
}
