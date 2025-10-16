use anyhow::Result;
use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    text::Text,
    widgets::Widget,
};

use super::Section;
use crate::{ctx::Ctx, shared::ext::rect::RectExt};

#[derive(derive_more::Debug, Default)]
pub struct SelectSection {
    pub items: Vec<SelectItem>,
    pub area: Rect,
    pub selected_idx: Option<usize>,
    pub current_item_style: Style,
    #[debug(skip)]
    pub on_confirm: Option<Box<dyn FnOnce(&Ctx, String) -> Result<()> + Send + Sync + 'static>>,
}

#[derive(derive_more::Debug)]
pub struct SelectItem {
    pub label: String,
    pub value: String,
}

impl SelectSection {
    pub fn new(current_item_style: Style) -> Self {
        Self {
            items: Vec::new(),
            area: Rect::default(),
            selected_idx: None,
            current_item_style,
            on_confirm: None,
        }
    }

    pub fn action(
        &mut self,
        on_confirm: impl FnOnce(&Ctx, String) -> Result<()> + Send + Sync + 'static,
    ) -> &mut Self {
        self.on_confirm = Some(Box::new(on_confirm));
        self
    }

    pub fn add_item(&mut self, label: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.items.push(SelectItem { label: label.into(), value: value.into() });
        self
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

impl Section for SelectSection {
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

    fn select(&mut self, idx: usize) {
        self.selected_idx = Some(idx);
    }

    fn unselect(&mut self) {
        self.selected_idx = None;
    }

    fn confirm(&mut self, ctx: &Ctx) -> Result<bool> {
        if let Some(selected_idx) = self.selected_idx
            && let Some(cb) = self.on_confirm.take()
        {
            (cb)(ctx, std::mem::take(&mut self.items[selected_idx].value))?;
        }
        Ok(false)
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn preferred_height(&self) -> u16 {
        self.items.len() as u16
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, filter: Option<&str>, ctx: &Ctx) {
        self.area = area;

        for (idx, item) in self.items.iter().enumerate() {
            let mut text = Text::raw(&item.label);

            if self.selected_idx.is_some_and(|i| i == idx) {
                text = text.style(self.current_item_style);
            } else if let Some(f) = filter
                && item.label.to_lowercase().contains(f)
            {
                text = text.style(ctx.config.theme.highlighted_item_style);
            }

            let mut item_area = area.shrink_from_top(idx as u16);
            item_area.height = 1;
            text.render(item_area, buf);
        }
    }

    fn left_click(&mut self, position: Position) {
        self.select_item_at_position(position);
    }

    fn double_click(&mut self, _pos: Position, ctx: &Ctx) -> Result<bool> {
        self.confirm(ctx)?;
        Ok(false)
    }

    fn find_next(&self, filter: &str) -> Option<usize> {
        let start = self.selected_idx.map_or(0, |s| s + 1);
        (start..self.items.len()).find(|&i| self.items[i].label.to_lowercase().contains(filter))
    }

    fn find_prev(&self, filter: &str) -> Option<usize> {
        let selected_idx = self.selected_idx.unwrap_or(self.items.len());
        (0..selected_idx).rev().find(|&i| self.items[i].label.to_lowercase().contains(filter))
    }

    fn find_first(&self, filter: &str) -> Option<usize> {
        (0..self.items.len()).find(|&i| self.items[i].label.to_lowercase().contains(filter))
    }
}
