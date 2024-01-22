use ratatui::{
    prelude::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};
use strum::{IntoEnumIterator, VariantNames};

use crate::{
    config::Config,
    mpd::commands::{status::OnOffOneshot, State},
};

use super::app_tabs::AppTabs;

pub struct Header<'a, T>
where
    T: PartialEq + IntoEnumIterator + VariantNames,
{
    config: &'a Config,
    active_tab: T,
    state: State,
    frame_count: u32,
    title: Option<&'a str>,
    artist: Option<&'a str>,
    album: Option<&'a str>,
    volume: u8,
    repeat: bool,
    random: bool,
    single: OnOffOneshot,
    consume: OnOffOneshot,
    elapsed: String,
    duration: String,
    bitrate: String,
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

        // right
        let volume = crate::ui::widgets::volume::Volume::default()
            .value(self.volume)
            .alignment(Alignment::Right)
            .style(Style::default().fg(config.ui.volume_color));

        let on_style = Style::default().fg(Color::Gray);
        let off_style = Style::default().fg(Color::DarkGray);
        let separator = Span::styled(" / ", on_style);
        let playback_states = Paragraph::new(Line::from(vec![
            Span::styled("Repeat", if self.repeat { on_style } else { off_style }),
            separator.clone(),
            Span::styled("Random", if self.random { on_style } else { off_style }),
            separator.clone(),
            match self.consume {
                OnOffOneshot::On => Span::styled("Consume", on_style),
                OnOffOneshot::Off => Span::styled("Consume", off_style),
                OnOffOneshot::Oneshot => Span::styled("Oneshot(C)", on_style),
            },
            separator,
            match self.single {
                OnOffOneshot::On => Span::styled("Single", on_style),
                OnOffOneshot::Off => Span::styled("Single", off_style),
                OnOffOneshot::Oneshot => Span::styled("Oneshot(S)", on_style),
            },
        ]))
        .alignment(Alignment::Right);

        // center
        let song_name = Paragraph::new(self.title.unwrap_or("No song"))
            .style(Style::default().bold())
            .alignment(Alignment::Center);

        // left
        // no rendered frames in release mode
        #[cfg(debug_assertions)]
        let status = Paragraph::new(Span::styled(
            format!("[{}] {} rendered frames", self.state, self.frame_count),
            Style::default().fg(config.ui.status_color),
        ));
        #[cfg(not(debug_assertions))]
        let status = Paragraph::new(Span::styled(
            format!("[{}]", app.status.state),
            Style::default().fg(app.config.ui.status_color),
        ));

        let elapsed = if config.status_update_interval_ms.is_some() {
            Paragraph::new(format!("{}/{}{}", self.elapsed, self.duration, self.bitrate))
        } else {
            Paragraph::new(format!("{}{}", self.duration, self.bitrate))
        }
        .style(Style::default().fg(Color::Gray));

        let song_info = Paragraph::new(Line::from(vec![
            Span::styled(self.artist.unwrap_or("Unknown"), Style::default().fg(Color::Yellow)),
            Span::styled(" - ", Style::default().bold()),
            ratatui::text::Span::styled(
                self.album.unwrap_or("Unknown Album"),
                Style::default().fg(Color::LightBlue),
            ),
        ]))
        .alignment(Alignment::Center);

        playback_states.render(bottom_right, buf);
        status.render(top_left, buf);
        elapsed.render(bottom_left, buf);
        volume.render(top_right, buf);
        song_name.render(top_center, buf);
        song_info.render(bottom_center, buf);

        let app_tabs = AppTabs::new(self.active_tab, config);
        app_tabs.render(tabs, buf);
    }
}

impl<'a, T> Header<'a, T>
where
    T: PartialEq + IntoEnumIterator + VariantNames,
{
    pub fn new(config: &'a Config, active_tab: T) -> Self {
        Self {
            config,
            state: State::Stop,
            frame_count: 0,
            title: None,
            artist: None,
            album: None,
            volume: 0,
            repeat: false,
            random: false,
            single: OnOffOneshot::Off,
            consume: OnOffOneshot::Off,
            active_tab,
            elapsed: String::new(),
            duration: String::new(),
            bitrate: String::new(),
        }
    }

    pub fn set_state(mut self, state: State) -> Self {
        self.state = state;
        self
    }

    pub fn set_frame_count(mut self, frame_count: u32) -> Self {
        self.frame_count = frame_count;
        self
    }

    pub fn set_title(mut self, title: Option<&'a str>) -> Self {
        self.title = title;
        self
    }

    pub fn set_artist(mut self, artist: Option<&'a str>) -> Self {
        self.artist = artist;
        self
    }

    pub fn set_album(mut self, album: Option<&'a str>) -> Self {
        self.album = album;
        self
    }

    pub fn set_volume(mut self, volume: u8) -> Self {
        self.volume = volume;
        self
    }

    pub fn set_repeat(mut self, repeat: bool) -> Self {
        self.repeat = repeat;
        self
    }

    pub fn set_random(mut self, random: bool) -> Self {
        self.random = random;
        self
    }

    pub fn set_single(mut self, single: OnOffOneshot) -> Self {
        self.single = single;
        self
    }

    pub fn set_consume(mut self, consume: OnOffOneshot) -> Self {
        self.consume = consume;
        self
    }

    pub fn set_active_tab(mut self, active_tab: T) -> Self {
        self.active_tab = active_tab;
        self
    }

    pub fn set_elapsed(mut self, elapsed: String) -> Self {
        self.elapsed = elapsed;
        self
    }

    pub fn set_duration(mut self, duration: String) -> Self {
        self.duration = duration;
        self
    }

    pub fn set_bitrate(mut self, bitrate: String) -> Self {
        self.bitrate = bitrate;
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
        let [header, tabs] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Length(3)].as_ref())
            .split(area)
        else {
            return None;
        };

        let [left_area, center_area, right_area] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(20),
                    Constraint::Percentage(60),
                    Constraint::Percentage(20),
                ]
                .as_ref(),
            )
            .split(header)
        else {
            return None;
        };

        let [top_center, bottom_center] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(center_area.height / 2),
                    Constraint::Length(center_area.height / 2),
                ]
                .as_ref(),
            )
            .split(center_area)
        else {
            return None;
        };

        let [top_right, bottom_right] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(center_area.height / 2),
                    Constraint::Length(center_area.height / 2),
                ]
                .as_ref(),
            )
            .split(right_area)
        else {
            return None;
        };

        let [top_left, bottom_left] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(center_area.height / 2),
                    Constraint::Length(center_area.height / 2),
                ]
                .as_ref(),
            )
            .split(left_area)
        else {
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
