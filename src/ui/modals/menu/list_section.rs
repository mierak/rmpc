#![allow(clippy::cast_possible_truncation)]
use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    text::Text,
    widgets::Widget,
};

use super::Section;
use crate::{ctx::Ctx, shared::ext::rect::RectExt};

#[derive(Debug, Default)]
pub struct ListSection {
    pub items: Vec<MenuItem>,
    pub area: Rect,
    pub selected_idx: Option<usize>,
    pub current_item_style: Style,
}

#[derive(derive_more::Debug)]
pub struct MenuItem {
    pub label: String,
    #[debug(skip)]
    pub on_confirm: Option<Box<dyn FnOnce(&Ctx) + Send + Sync + 'static>>,
}

impl ListSection {
    pub fn new(current_item_style: Style) -> Self {
        Self { items: Vec::new(), area: Rect::default(), selected_idx: None, current_item_style }
    }

    pub fn add_item(
        mut self,
        label: impl Into<String>,
        on_confirm: impl FnOnce(&Ctx) + Send + Sync + 'static,
    ) -> Self {
        self.items.push(MenuItem { label: label.into(), on_confirm: Some(Box::new(on_confirm)) });
        self
    }

    pub fn item_at_position(&mut self, position: Position) -> Option<&mut MenuItem> {
        if !self.area.contains(position) {
            return None;
        }

        let idx = position.y.saturating_sub(self.area.y) as usize;
        self.items.get_mut(idx)
    }

    pub fn select_item_at_position(&mut self, position: Position) {
        if !self.area.contains(position) {
            return;
        }

        let idx = position.y.saturating_sub(self.area.y) as usize;
        if idx < self.items.len() {
            self.selected_idx = Some(idx);
        } else {
            self.selected_idx = None;
        }
    }
}

impl Section for ListSection {
    fn down(&mut self) -> bool {
        self.selected_idx = match self.selected_idx {
            Some(idx) if idx + 1 == self.items.len() => None,
            Some(idx) => Some(idx + 1),
            None => Some(0),
        };

        self.selected_idx.is_some()
    }

    fn up(&mut self) -> bool {
        self.selected_idx = match self.selected_idx {
            Some(0) => None,
            Some(idx) => Some(idx.saturating_sub(1)),
            None => Some(self.items.len().saturating_sub(1)),
        };

        self.selected_idx.is_some()
    }

    fn unselect(&mut self) {
        self.selected_idx = None;
    }

    fn confirm(&mut self, ctx: &Ctx) -> bool {
        if let Some(selected_idx) = self.selected_idx {
            if let Some(cb) = self.items[selected_idx].on_confirm.take() {
                (cb)(ctx);
            }
        }
        false
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        Widget::render(self, area, buf);
    }

    fn left_click(&mut self, position: Position) {
        self.select_item_at_position(position);
    }

    fn double_click(&mut self, _pos: Position, ctx: &Ctx) -> bool {
        self.confirm(ctx);
        false
    }
}

impl Widget for &mut ListSection {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.area = area;

        for (idx, item) in self.items.iter().enumerate() {
            let mut text = Text::raw(&item.label);

            if self.selected_idx.is_some_and(|i| i == idx) {
                text = text.style(self.current_item_style);
            }

            let mut item_area = area.shrink_from_top(idx as u16);
            item_area.height = 1;
            text.render(item_area, buf);
        }
    }
}
