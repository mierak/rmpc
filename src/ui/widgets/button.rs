use ratatui::{
    layout::{Position, Rect},
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

impl Default for Button<'_> {
    fn default() -> Self {
        Self { label_alignment: Alignment::Center, label: "", block: None, style: Style::default() }
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

    pub fn set_style(&mut self, style: Style) {
        self.style = style;
    }
}

impl Widget for Button<'_> {
    fn render(mut self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        (&mut self).render(area, buf);
    }
}

impl Widget for &mut Button<'_> {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        buf.set_style(area, self.style);
        let area = match &self.block {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        buf.set_string(
            area.left()
                + get_line_offset(self.label.len() as u16, area.width, self.label_alignment),
            area.top(),
            self.label,
            self.style,
        );
    }
}

#[derive(Default, Debug)]
pub struct ButtonGroupState {
    pub selected: usize,
    button_count: usize,
}

#[derive(Debug)]
pub struct ButtonGroup<'a> {
    pub buttons: Vec<Button<'a>>,
    block: Option<Block<'a>>,
    active_style: Style,
    inactive_style: Style,
    direction: Direction,
    pub areas: Vec<Rect>,
}

impl Default for ButtonGroup<'_> {
    fn default() -> Self {
        Self {
            buttons: Vec::new(),
            block: None,
            active_style: Style::default().reversed(),
            inactive_style: Style::default(),
            direction: Direction::Horizontal,
            areas: Vec::default(),
        }
    }
}

impl StatefulWidget for &mut ButtonGroup<'_> {
    type State = ButtonGroupState;

    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut Self::State,
    ) {
        // buf.set_style(area, self.inactive_style);
        let area = match &self.block {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        let button_count = self.buttons.len();
        let portion_per_button = (100.0 / button_count as f32).floor() as usize;
        let constraints: Vec<Constraint> =
            (0..button_count).map(|_| Constraint::Percentage(portion_per_button as u16)).collect();

        let chunks =
            Layout::default().direction(self.direction).constraints(constraints).split(area);

        self.buttons.iter_mut().enumerate().for_each(|(idx, button)| {
            if idx == state.selected {
                button.set_style(self.active_style);
            } else {
                button.set_style(self.inactive_style);
            }
            self.areas[idx] = chunks[idx];
            button.render(chunks[idx], buf);
        });
    }
}

#[allow(dead_code)]
impl<'a> ButtonGroup<'a> {
    pub fn buttons(mut self, buttons: Vec<Button<'a>>) -> Self {
        self.areas = vec![Rect::default(); buttons.len()];
        self.buttons = buttons;
        self
    }

    pub fn add_button(mut self, button: Button<'a>) -> Self {
        self.buttons.push(button);
        self.areas.push(Rect::default());
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

    pub fn set_active_style(&mut self, style: Style) {
        self.active_style = style;
    }

    pub fn get_button_idx_at(&self, position: Position) -> Option<usize> {
        self.areas.iter().enumerate().find(|(_, area)| area.contains(position)).map(|v| v.0)
    }
}

impl ButtonGroupState {
    pub fn button_count(&mut self) -> usize {
        self.button_count
    }

    pub fn set_button_count(&mut self, count: usize) -> &Self {
        self.button_count = count;
        self
    }

    pub fn select(&mut self, value: usize) {
        self.selected = value.min(self.button_count);
    }

    pub fn next(&mut self) {
        if self.button_count == 0 {
            return;
        }
        self.selected = self.selected.saturating_add(1);
        if self.selected > self.button_count - 1 {
            self.selected = 0;
        }
    }

    pub fn prev(&mut self) {
        if self.selected == 0 {
            self.selected = self.button_count.saturating_sub(1);
        } else {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    pub fn first(&mut self) {
        self.selected = 0;
    }

    pub fn last(&mut self) {
        self.selected = self.button_count.saturating_sub(1);
    }
}
