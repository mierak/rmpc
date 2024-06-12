use ratatui::{
    prelude::{Alignment, Constraint, Direction, Layout},
    style::{Style, Stylize},
    widgets::{Block, StatefulWidget, Widget},
};

use super::get_line_offset;

#[derive(Debug)]
pub struct Button<'a> {
    label: &'a str,
    block: Option<Block<'a>>,
    style: Style,
    label_alignment: Alignment,
}

impl<'a> Default for Button<'a> {
    fn default() -> Self {
        Self {
            label_alignment: Alignment::Center,
            label: "",
            block: None,
            style: Style::default(),
        }
    }
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

    pub fn label_alignment(mut self, alignment: Alignment) -> Self {
        self.label_alignment = alignment;
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
            area.left() + get_line_offset(self.label.len() as u16, area.width, self.label_alignment),
            area.top(),
            self.label,
            self.style,
        );
    }
}

#[derive(Default, Debug)]
pub struct ButtonGroupState {
    pub selected: usize,
    button_conut: usize,
}

#[derive(Debug)]
pub struct ButtonGroup<'a> {
    buttons: Vec<Button<'a>>,
    block: Option<Block<'a>>,
    active_style: Style,
    inactive_style: Style,
    direction: Direction,
}

impl<'a> Default for ButtonGroup<'a> {
    fn default() -> Self {
        Self {
            buttons: Vec::new(),
            block: None,
            active_style: Style::default().reversed(),
            inactive_style: Style::default(),
            direction: Direction::Horizontal,
        }
    }
}

impl<'a> StatefulWidget for ButtonGroup<'a> {
    type State = ButtonGroupState;

    fn render(mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer, state: &mut Self::State) {
        // buf.set_style(area, self.inactive_style);
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
                button = button.style(self.active_style);
            } else {
                button = button.style(self.inactive_style);
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

    pub fn add_button(mut self, button: Button<'a>) -> Self {
        self.buttons.push(button);
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

    pub fn active_style(mut self, style: Style) -> Self {
        self.active_style = style;
        self
    }
    pub fn inactive_style(mut self, style: Style) -> Self {
        self.inactive_style = style;
        self
    }
}

impl ButtonGroupState {
    pub fn button_count(&mut self) -> usize {
        self.button_conut
    }

    pub fn set_button_count(&mut self, count: usize) -> &Self {
        self.button_conut = count;
        self
    }

    pub fn next(&mut self) {
        // todo handle empty buttons
        self.selected = self.selected.saturating_add(1);
        if self.selected > self.button_conut - 1 {
            self.selected = 0;
        }
    }

    pub fn prev(&mut self) {
        if self.selected == 0 {
            self.selected = self.button_conut - 1;
        } else {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    pub fn first(&mut self) {
        self.selected = 0;
    }

    pub fn last(&mut self) {
        self.selected = self.button_conut.saturating_sub(1);
    }
}
