use anyhow::Result;
use enum_map::{Enum, EnumMap};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Position, Rect},
    style::Style,
    text::Text,
    widgets::{ListState, StatefulWidget, Widget},
};

use super::Section;
use crate::{ctx::Ctx, shared::ext::rect::RectExt, ui::dirstack::DirState};

#[derive(derive_more::Debug, Default)]
pub struct SelectSection {
    pub items: Vec<SelectItem>,
    pub areas: EnumMap<ListSectionArea, Rect>,
    pub current_item_style: Style,
    #[debug(skip)]
    pub on_confirm: Option<Box<dyn FnOnce(&Ctx, String) -> Result<()> + Send + Sync + 'static>>,
    max_height: Option<usize>,
    state: DirState<ListState>,
}

#[derive(Copy, Clone, Debug, Enum, Eq, PartialEq, Hash)]
pub enum ListSectionArea {
    List = 0,
    Scrollbar = 1,
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
            areas: EnumMap::default(),
            current_item_style,
            on_confirm: None,
            max_height: None,
            state: DirState::default(),
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

    pub fn add_max_height(&mut self, height: usize) -> &mut Self {
        self.max_height = Some(height);
        self
    }

    pub fn select_item_at_position(&mut self, position: Position) {
        if !self.areas[ListSectionArea::List].contains(position) {
            return;
        }

        let clicked_row: usize =
            position.y.saturating_sub(self.areas[ListSectionArea::List].y).into();
        let idx = self.state.get_at_rendered_row(clicked_row);
        self.state.select(idx, 0);
    }
}

impl Section for SelectSection {
    fn down(&mut self) -> bool {
        let initial_selected = self.state.get_selected();
        self.state.next(0, false);

        if let Some(init) = initial_selected
            && init == self.items.len().saturating_sub(1)
            && self.state.get_selected().is_some()
        {
            let offset = self.state.offset();
            self.state.inner.select(None);
            self.state.set_offset(offset);
            return false;
        }
        true
    }

    fn up(&mut self) -> bool {
        let initial_selected = self.state.get_selected();
        self.state.prev(0, true);

        if let Some(init) = initial_selected
            && init == 0
            && self.state.get_selected().is_some()
        {
            self.state.inner.select(None);
            self.state.set_offset(0);
            return false;
        }
        true
    }

    fn selected(&self) -> Option<usize> {
        self.state.get_selected()
    }

    fn select(&mut self, idx: usize) {
        self.state.select(Some(idx), 0);
    }

    fn unselect(&mut self, _ctx: &Ctx) {
        let offset = self.state.offset();
        self.state.inner.select(None);
        self.state.set_offset(offset);
    }

    fn confirm(&mut self, ctx: &Ctx) -> Result<()> {
        if let Some(selected_idx) = self.state.get_selected()
            && let Some(cb) = self.on_confirm.take()
        {
            (cb)(ctx, std::mem::take(&mut self.items[selected_idx].value))?;
        }
        Ok(())
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn preferred_height(&self) -> u16 {
        let len = self.items.len();
        self.max_height.map_or(len, |mh| len.min(mh)) as u16
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, filter: Option<&str>, ctx: &Ctx) {
        let should_show_scrollbar = ctx.config.as_styled_scrollbar().is_some()
            && self.max_height.is_some_and(|h| h < self.items.len());

        let [list_area, scrolling_area] = if should_show_scrollbar {
            Layout::horizontal([Constraint::Percentage(100), Constraint::Min(1)]).areas(area)
        } else {
            [area, Rect::default()]
        };
        self.areas[ListSectionArea::List] = list_area;
        self.areas[ListSectionArea::Scrollbar] = scrolling_area;

        let list_area = self.areas[ListSectionArea::List];
        self.state.set_content_and_viewport_len(self.items.len(), list_area.height as usize);
        for (idx, item) in self
            .items
            .iter()
            .enumerate()
            .skip(self.state.offset())
            .take(self.max_height.unwrap_or(usize::MAX))
        {
            let mut text = Text::raw(&item.label);

            if self.state.get_selected().is_some_and(|i| i == idx) {
                text = text.style(self.current_item_style);
            } else if let Some(f) = filter
                && item.label.to_lowercase().contains(f)
            {
                text = text.style(ctx.config.theme.highlighted_item_style);
            }
            let idx = idx.saturating_sub(self.state.offset());

            let mut item_area = list_area.shrink_from_top(idx as u16);
            item_area.height = 1;
            text.render(item_area, buf);
        }

        if self.areas[ListSectionArea::Scrollbar].width > 0
            && let Some(scrollbar) = ctx.config.as_styled_scrollbar()
        {
            scrollbar.render(
                self.areas[ListSectionArea::Scrollbar],
                buf,
                self.state.as_scrollbar_state_ref(),
            );
        }
    }

    fn left_click(&mut self, position: Position, _ctx: &Ctx) {
        self.select_item_at_position(position);
    }

    fn double_click(&mut self, _pos: Position, ctx: &Ctx) -> Result<bool> {
        self.confirm(ctx)?;
        Ok(false)
    }

    fn item_labels_iter(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        Box::new(self.items.iter().map(|i| i.label.as_str()))
    }
}
