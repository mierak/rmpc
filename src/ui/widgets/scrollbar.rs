use std::ops::Range;

use ratatui::{
    style::Style,
    widgets::{scrollbar::Set, ScrollDirection, ScrollbarOrientation, StatefulWidget},
};

#[derive(Debug, Default)]
pub struct ScrollbarState {
    position: u16,
    content_length: u16,
    viewport_content_length: u16,
    offset: u16,
    pub inner: ratatui::widgets::ScrollbarState,
}

#[allow(dead_code)]
impl ScrollbarState {
    pub fn position(&mut self, position: u16) -> &Self {
        self.position = position;
        self.inner = self.inner.position(position);
        self.calculate_offset();
        self
    }

    pub fn content_length(&mut self, content_length: u16) -> &Self {
        self.content_length = content_length;
        self.inner = self.inner.content_length(content_length);
        if self.position >= self.content_length {
            self.position(content_length.saturating_sub(1));
        }
        self.calculate_offset();
        self
    }

    pub fn viewport_content_length(&mut self, viewport_content_length: u16) -> &Self {
        self.viewport_content_length = viewport_content_length;
        self.inner = self.inner.viewport_content_length(viewport_content_length);
        self.calculate_offset();
        self
    }

    pub fn prev(&mut self) {
        // wrap around
        if self.position == 0 {
            self.position = self.content_length.saturating_sub(1);
        } else {
            self.position = self.position.saturating_sub(1);
        }
        self.inner = self.inner.position(self.position);
        self.calculate_offset();
    }

    pub fn next(&mut self) {
        self.position = self.position.saturating_add(1);
        // wrap around
        if self.position > self.content_length.saturating_sub(1) {
            self.position = 0;
        }
        self.inner = self.inner.position(self.position);
        self.calculate_offset();
    }

    pub fn first(&mut self) {
        self.position = 0;
        self.inner.first();
        self.calculate_offset();
    }

    pub fn last(&mut self) {
        self.position = self.content_length.saturating_sub(1);
        self.inner.last();
        self.calculate_offset();
    }

    pub fn scroll(&mut self, direction: ScrollDirection) {
        match direction {
            ScrollDirection::Forward => {
                self.next();
            }
            ScrollDirection::Backward => {
                self.prev();
            }
        }
        self.inner.scroll(direction);
        self.calculate_offset();
    }

    fn is_in_viewport(&self) -> bool {
        self.position < self.offset + self.viewport_content_length && self.position >= self.offset
    }

    pub fn calculate_offset(&mut self) {
        if self.offset + self.viewport_content_length > self.content_length {
            self.offset = self.content_length.saturating_sub(self.viewport_content_length);
            return;
        }

        if self.content_length <= self.viewport_content_length {
            self.offset = 0;
            return;
        }

        if self.is_in_viewport() {
            return;
        }

        if self.position == self.offset.saturating_sub(1) {
            self.offset = self.offset.saturating_sub(1);
            return;
        }

        if self.position == self.offset + self.viewport_content_length {
            self.offset = self.offset.saturating_add(1);
            return;
        }

        self.offset = self
            .position
            .saturating_sub(self.viewport_content_length.saturating_sub(1))
    }

    pub fn get_position(&self) -> u16 {
        self.position
    }

    pub fn get_content_length(&self) -> u16 {
        self.content_length
    }

    pub fn get_offset(&self) -> u16 {
        self.offset
    }

    pub fn get_range(&self) -> Range<u16> {
        self.offset..self.offset + self.viewport_content_length
    }

    pub fn get_range_usize(&self) -> Range<usize> {
        (self.offset as usize).max(0)..((self.offset + self.viewport_content_length).min(self.content_length) as usize)
    }
}

#[derive(Default)]
pub struct Scrollbar<'a> {
    inner: ratatui::widgets::Scrollbar<'a>,
}

impl StatefulWidget for Scrollbar<'_> {
    type State = ratatui::widgets::ScrollbarState;

    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer, state: &mut Self::State) {
        self.inner.render(area, buf, state);
    }
}

#[allow(dead_code)]
impl<'a> Scrollbar<'a> {
    pub fn orientation(mut self, orientation: ScrollbarOrientation) -> Self {
        self.inner = self.inner.orientation(orientation);
        self
    }

    pub fn thumb_symbol(mut self, thumb_symbol: &'a str) -> Self {
        self.inner = self.inner.thumb_symbol(thumb_symbol);
        self
    }

    pub fn thumb_style(mut self, thumb_style: Style) -> Self {
        self.inner = self.inner.thumb_style(thumb_style);
        self
    }

    pub fn track_symbol(mut self, track_symbol: &'a str) -> Self {
        self.inner = self.inner.track_symbol(track_symbol);
        self
    }

    pub fn track_style(mut self, track_style: Style) -> Self {
        self.inner = self.inner.track_style(track_style);
        self
    }

    pub fn begin_symbol(mut self, begin_symbol: Option<&'a str>) -> Self {
        self.inner = self.inner.begin_symbol(begin_symbol);
        self
    }

    pub fn begin_style(mut self, begin_style: Style) -> Self {
        self.inner = self.inner.begin_style(begin_style);
        self
    }

    pub fn end_symbol(mut self, end_symbol: Option<&'a str>) -> Self {
        self.inner = self.inner.end_symbol(end_symbol);
        self
    }

    pub fn end_style(mut self, end_style: Style) -> Self {
        self.inner = self.inner.end_style(end_style);
        self
    }

    pub fn symbols(mut self, symbol: Set) -> Self {
        self.inner = self.inner.symbols(symbol);
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.inner = self.inner.style(style);
        self
    }
}
