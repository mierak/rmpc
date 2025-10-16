use anyhow::Result;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Position, Rect},
    style::Style,
    text::Text,
    widgets::{StatefulWidget, Widget},
};

use super::Section;
use crate::{
    ctx::Ctx,
    shared::ext::rect::RectExt as _,
    ui::widgets::button::{Button, ButtonGroup, ButtonGroupState},
};

#[derive(derive_more::Debug, Default)]
pub struct MultiActionSection<'a> {
    pub items: Vec<MultiActionItem<'a>>,
    pub area: Rect,
    pub selected_idx: Option<usize>,
    pub current_item_style: Style,
    #[debug(skip)]
    pub actions: Vec<(&'a str, Option<Box<dyn FnOnce(&Ctx, String) + Send + Sync + 'static>>)>,
}

#[derive(derive_more::Debug)]
pub struct MultiActionItem<'a> {
    pub label: String,
    pub buttons: ButtonGroup<'a>,
    pub buttons_state: ButtonGroupState,
}

impl<'a> MultiActionSection<'a> {
    pub fn new(current_item_style: Style) -> Self {
        Self {
            items: Vec::new(),
            area: Rect::default(),
            selected_idx: None,
            current_item_style,
            actions: Vec::new(),
        }
    }

    pub fn add_item(mut self, label: impl Into<String>) -> Self {
        self.items.push(MultiActionItem {
            label: label.into(),
            buttons: ButtonGroup::default().spacing(1),
            buttons_state: ButtonGroupState::default(),
        });
        self
    }

    pub fn add_action(
        mut self,
        label: &'a str,
        action: impl FnOnce(&Ctx, String) + Send + Sync + 'static,
    ) -> Self {
        self.actions.push((label, Some(Box::new(action))));
        self
    }

    pub fn build(&mut self) {
        for item in &mut self.items {
            let mut buttons = std::mem::take(&mut item.buttons);
            for action in &mut self.actions {
                buttons = buttons.add_button(Button::default().label(action.0));
            }
            item.buttons = buttons;
        }
    }
}

impl Section for MultiActionSection<'_> {
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

    fn right(&mut self) -> bool {
        if let Some(idx) = self.selected_idx {
            self.items[idx].buttons_state.next();
        }

        true
    }

    fn left(&mut self) -> bool {
        if let Some(idx) = self.selected_idx {
            self.items[idx].buttons_state.prev();
        }

        true
    }

    fn unselect(&mut self) {
        self.selected_idx = None;
    }

    fn confirm(&mut self, ctx: &Ctx) -> Result<bool> {
        if let Some(selected_idx) = self.selected_idx {
            let label = std::mem::take(&mut self.items[selected_idx].label);
            let selected_button = self.items[selected_idx].buttons_state.selected;
            if let Some(cb) = self.actions[selected_button].1.take() {
                (cb)(ctx, label);
            }
        }

        Ok(false)
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn preferred_height(&self) -> u16 {
        self.items.len() as u16
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _ctx: &Ctx) {
        self.area = area;

        for (idx, item) in self.items.iter_mut().enumerate() {
            let mut text = Text::raw(&item.label);

            if self.selected_idx.is_some_and(|i| i == idx) {
                text = text.style(self.current_item_style);
            }

            let mut item_area = area.shrink_from_top(idx as u16);
            item_area.height = 1;
            let [label_area, buttons_area] =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .spacing(1)
                    .areas(item_area);

            text.render(label_area, buf);

            if self.selected_idx.is_some_and(|i| i == idx) {
                item.buttons.set_active_style(self.current_item_style);
            } else {
                item.buttons.set_active_style(Style::default());
            }

            item.buttons.render(buttons_area, buf, &mut item.buttons_state);
        }
    }

    fn left_click(&mut self, position: Position) {
        if !self.area.contains(position) {
            return;
        }

        let items_len = self.items.len();
        for item in &mut self.items {
            let idx = position.y.saturating_sub(self.area.y) as usize;
            if idx < items_len {
                self.selected_idx = Some(idx);
            } else {
                self.selected_idx = None;
            }

            let res = item.buttons.get_button_idx_at(position);
            if let Some(idx) = res {
                item.buttons_state.select(idx);
            }
        }
    }

    fn double_click(&mut self, _pos: Position, ctx: &Ctx) -> Result<bool> {
        self.confirm(ctx)?;
        Ok(false)
    }
}
