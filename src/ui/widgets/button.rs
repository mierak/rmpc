use ratatui::{
    prelude::{Alignment, Constraint, Direction, Layout},
    style::{Style, Stylize},
    widgets::{Block, StatefulWidget, Widget},
};

use super::get_line_offset;

#[derive(Default, Debug)]
pub struct Button<'a> {
    label: &'a str,
    block: Option<Block<'a>>,
    style: Style,
}

#[allow(dead_code)]
impl<'a> Button<'a> {
    pub fn label(mut self, label: &'a str) -> Self {
        self.label = label;
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
}

impl<'a> Widget for Button<'a> {
    fn render(mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        buf.set_style(area, self.style);
        let area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        buf.set_string(
            area.left() + get_line_offset(self.label.len() as u16, area.width, Alignment::Center),
            area.top(),
            self.label,
            self.style,
        );
    }
}

#[derive(Default, Debug)]
pub struct ButtonGroupState {
    pub selected: usize,
}

#[derive(Debug)]
pub struct ButtonGroup<'a> {
    buttons: Vec<Button<'a>>,
    block: Option<Block<'a>>,
    style: Style,
    direction: Direction,
}

impl<'a> Default for ButtonGroup<'a> {
    fn default() -> Self {
        Self {
            buttons: Vec::new(),
            block: None,
            style: Style::default(),
            direction: Direction::Horizontal,
        }
    }
}

impl<'a> StatefulWidget for ButtonGroup<'a> {
    type State = ButtonGroupState;

    fn render(mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer, state: &mut Self::State) {
        buf.set_style(area, self.style);
        let area = match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        let button_count = self.buttons.len();
        let portion_per_button = (100.0 / button_count as f32).floor() as usize;
        let constraints: Vec<Constraint> = (0..button_count)
            .map(|_| Constraint::Percentage(portion_per_button as u16))
            .collect();

        let chunks = Layout::default()
            .direction(self.direction)
            .constraints(constraints)
            .split(area);

        self.buttons.into_iter().enumerate().for_each(|(idx, button)| {
            let mut button = button;
            if idx == state.selected {
                let style = button.style;
                button = button.style(style.reversed());
            }
            button.render(chunks[idx], buf);
        });
    }
}

#[allow(dead_code)]
impl<'a> ButtonGroup<'a> {
    pub fn buttons(mut self, buttons: Vec<Button<'a>>) -> Self {
        self.buttons = buttons;
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}
