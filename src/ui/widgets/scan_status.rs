use ratatui::{
    prelude::Alignment,
    style::Style,
    widgets::{Block, StatefulWidget, Widget},
};

use super::get_line_offset;

const DEFAULT_LOADING_CHARS: [&str; 8] = ["⣻", "⣽", "⣾", "⣷", "⣯", "⣟", "⡿", "⢿"];

#[derive(Default, Debug)]
pub struct ScanStatusState {
    pub updating: bool,
    pub symbol_index: usize,
}

impl ScanStatusState {
    pub fn cycle_load_symbol(&mut self) {
        self.symbol_index = (self.symbol_index + 1) % DEFAULT_LOADING_CHARS.len();
    }
}

impl ScanStatusState {
    pub fn new(updating: Option<u32>) -> Self {
        Self { updating: updating.is_some(), symbol_index: 0 }
    }
}

#[derive(Debug)]
pub struct ScanStatus<'a> {
    block: Option<Block<'a>>,
    alignment: Alignment,
    style: Style,
}

impl Default for ScanStatus<'_> {
    fn default() -> Self {
        Self { block: None, alignment: Alignment::Left, style: Style::default() }
    }
}

#[allow(dead_code)]
impl<'a> ScanStatus<'a> {
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

    /// get updating symbol, this symbol rotates in set inverval if the db is
    /// scanning
    pub fn get_str(&mut self, state: &mut ScanStatusState) -> String {
        if !state.updating {
            return String::new();
        }
        // SAFETY: module of len guarantees the index is always inbound
        let t = unsafe { DEFAULT_LOADING_CHARS.get_unchecked(state.symbol_index) };
        state.cycle_load_symbol();
        format!(" {t} ")
    }
}

impl StatefulWidget for &mut ScanStatus<'_> {
    type State = ScanStatusState;

    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut Self::State,
    ) {
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

        buf.set_string(area.left() + left_offset, area.top(), self.get_str(state), self.style);
    }
}
