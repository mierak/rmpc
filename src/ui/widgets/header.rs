use ratatui::{
    prelude::{Constraint, Layout},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Widget},
};
use strum::{IntoEnumIterator, VariantNames};

use crate::{
    config::ui::properties::Property,
    config::Config,
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

        let Some(Layouts {
            top_left,
            top_center,
            top_right,
            bottom_left,
            bottom_center,
            bottom_right,
            tabs,
        }) = Layouts::new(area)
        else {
            return;
        };

        if let Some(header_bg_color) = config.ui.header_background_color {
            Block::default()
                .style(Style::default().bg(header_bg_color))
                .render(area, buf);
        }

        let c = &config.ui.header;

        let top_left_w = PropertyTemplates(&c.top_left);
        let top_left_w = top_left_w.format(self.song, self.status).left_aligned();
        top_left_w.render(top_left, buf);

        let top_center_w = PropertyTemplates(&c.top_center);
        let top_center_w = top_center_w.format(self.song, self.status).centered();
        top_center_w.render(top_center, buf);

        let top_right_w = PropertyTemplates(&c.top_right);
        let top_right_w = top_right_w.format(self.song, self.status).right_aligned();
        top_right_w.render(top_right, buf);

        let bot_left_w = PropertyTemplates(&c.bottom_left);
        let bot_left_w = bot_left_w.format(self.song, self.status).left_aligned();
        bot_left_w.render(bottom_left, buf);

        let bot_center_w = PropertyTemplates(&c.bottom_center);
        let bot_center_w = bot_center_w.format(self.song, self.status).centered();
        bot_center_w.render(bottom_center, buf);

        let bot_right_w = PropertyTemplates(&c.bottom_right);
        let bot_right_w = bot_right_w.format(self.song, self.status).right_aligned();
        bot_right_w.render(bottom_right, buf);

        let app_tabs = AppTabs::new(self.active_tab, config);
        app_tabs.render(tabs, buf);
    }
}

struct PropertyTemplates<'a>(&'a [Property]);
impl<'a> PropertyTemplates<'a> {
    fn format(&'a self, song: Option<&'a Song>, status: &'a Status) -> Line<'a> {
        Line::from(self.0.iter().fold(Vec::new(), |mut acc, val| {
            match *val {
                Property::Song(sp) => acc.push(sp.as_span_opt(song)),
                Property::Status(p) => acc.push(p.as_span(status)),
                Property::Widget(w) => acc.append(&mut w.as_spans(status)),
                Property::Text { value, style } => acc.push(Span::styled(value, style)),
            }
            acc
        }))
    }
}

impl<'a, T> Header<'a, T>
where
    T: PartialEq + IntoEnumIterator + VariantNames,
{
    pub fn new(config: &'a Config, active_tab: T, status: &'a Status) -> Self {
        Self {
            config,
            status,
            active_tab,
            song: None,
        }
    }

    pub fn set_song(mut self, song: Option<&'a Song>) -> Self {
        self.song = song;
        self
    }
}

struct Layouts {
    top_left: ratatui::prelude::Rect,
    top_center: ratatui::prelude::Rect,
    top_right: ratatui::prelude::Rect,
    bottom_left: ratatui::prelude::Rect,
    bottom_center: ratatui::prelude::Rect,
    bottom_right: ratatui::prelude::Rect,
    tabs: ratatui::prelude::Rect,
}

impl Layouts {
    fn new(area: ratatui::prelude::Rect) -> Option<Self> {
        let [header, tabs] = *Layout::vertical([Constraint::Length(2), Constraint::Length(3)]).split(area) else {
            return None;
        };

        let [left_area, center_area, right_area] = *Layout::horizontal([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(header) else {
            return None;
        };

        let [top_center, bottom_center] = *Layout::vertical([
            Constraint::Length(center_area.height / 2),
            Constraint::Length(center_area.height / 2),
        ])
        .split(center_area) else {
            return None;
        };

        let [top_right, bottom_right] = *Layout::vertical([
            Constraint::Length(center_area.height / 2),
            Constraint::Length(center_area.height / 2),
        ])
        .split(right_area) else {
            return None;
        };

        let [top_left, bottom_left] = *Layout::vertical([
            Constraint::Length(center_area.height / 2),
            Constraint::Length(center_area.height / 2),
        ])
        .split(left_area) else {
            return None;
        };

        Some(Self {
            top_left,
            top_center,
            top_right,
            bottom_left,
            bottom_center,
            bottom_right,
            tabs,
        })
    }
}
