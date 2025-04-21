use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::{
    prelude::Alignment,
    style::Style,
    widgets::{Block, Widget},
};

use super::get_line_offset;

// TODO: merge with config loader
const DEFAULT_LOADING_CHARS: [&str; 8] = ["⣻", "⣽", "⣾", "⣷", "⣯", "⣟", "⡿", "⢿"];

#[derive(Debug)]
pub struct ScanStatus<'a> {
    value: bool,
    symbol_index: usize,
    block: Option<Block<'a>>,
    alignment: Alignment,
    style: Style,
}

impl Default for ScanStatus<'_> {
    fn default() -> Self {
        Self {
            value: false,
            symbol_index: 0,
            block: None,
            alignment: Alignment::Left,
            style: Style::default(),
        }
    }
}

#[allow(dead_code)]
impl<'a> ScanStatus<'a> {
    pub fn value(mut self, value: bool) -> Self {
        self.value = value;
        self
    }

    pub fn cycle_load_symbol(mut self) -> Self {
        self.symbol_index = (self.symbol_index + 1) % DEFAULT_LOADING_CHARS.len();
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }
}

impl ScanStatus<'_> {
    pub fn get_str(updating: bool) -> String {
        if !updating {
            return String::new();
        }

        // TODO: figure out if we have a mechanism to track ticks instead of
        // instant
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("travel to the past is not possible")
            .as_secs();
        let i = secs % DEFAULT_LOADING_CHARS.len() as u64;
        // SAFETY: module of len guarantees the index is always inbound
        let t = unsafe { DEFAULT_LOADING_CHARS.get_unchecked(i as usize) };
        format!(" {t} ")
    }
}

impl Widget for ScanStatus<'_> {
    fn render(mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        if area.height < 1 {
            return;
        }

        let left_offset = get_line_offset(3, area.width, self.alignment);

        buf.set_string(
            area.left() + left_offset,
            area.top(),
            Self::get_str(self.value),
            self.style,
        );
    }
}
