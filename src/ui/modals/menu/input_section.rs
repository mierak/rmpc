use std::borrow::Cow;

use anyhow::Result;
use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    widgets::Widget,
};

use super::Section;
use crate::{
    ctx::Ctx,
    ui::{input::BufferId, widgets::input::Input},
};

#[derive(derive_more::Debug)]
pub struct InputSection<'a> {
    pub label: Cow<'a, str>,
    pub area: Rect,
    pub is_current: bool,
    #[debug(skip)]
    pub action: Option<Box<dyn FnOnce(&Ctx, String) + Send + Sync + 'static>>,
    pub buffer_id: BufferId,
}

impl<'a> InputSection<'a> {
    pub fn new(label: impl Into<Cow<'a, str>>) -> Self {
        Self {
            area: Rect::default(),
            label: label.into(),
            action: None,
            is_current: false,
            buffer_id: BufferId::new(),
        }
    }

    pub fn add_action(
        &mut self,
        action: impl FnOnce(&Ctx, String) + Send + Sync + 'static,
    ) -> &mut Self {
        self.action = Some(Box::new(action));
        self
    }

    pub fn action(mut self, action: impl FnOnce(&Ctx, String) + Send + Sync + 'static) -> Self {
        self.action = Some(Box::new(action));
        self
    }

    pub fn add_initial_value(&mut self, value: impl Into<String>, ctx: &Ctx) -> &mut Self {
        ctx.input.create_buffer(self.buffer_id, Some(&value.into()));
        self
    }
}

impl Section for InputSection<'_> {
    fn down(&mut self) -> bool {
        self.is_current = !self.is_current;
        self.is_current
    }

    fn up(&mut self) -> bool {
        self.is_current = !self.is_current;
        self.is_current
    }

    fn selected(&self) -> Option<usize> {
        if self.is_current { Some(0) } else { None }
    }

    fn select(&mut self, idx: usize) {
        self.is_current = idx == 0;
    }

    fn unfocus(&mut self, ctx: &Ctx) {
        if ctx.input.is_active(self.buffer_id) {
            ctx.input.normal_mode();
        }
    }

    fn unselect(&mut self, ctx: &Ctx) {
        if ctx.input.is_active(self.buffer_id) {
            ctx.input.normal_mode();
        }
        self.is_current = false;
    }

    fn confirm(&mut self, ctx: &Ctx) -> Result<()> {
        if ctx.input.is_active(self.buffer_id) {
            if let Some(cb) = self.action.take() {
                let value = ctx.input.value(self.buffer_id);
                (cb)(ctx, value);
            }
        } else {
            ctx.input.insert_mode(self.buffer_id);
        }
        Ok(())
    }

    fn on_close(&mut self, ctx: &Ctx) -> Result<()> {
        ctx.input.destroy_buffer(self.buffer_id);
        Ok(())
    }

    fn len(&self) -> usize {
        1
    }

    fn preferred_height(&self) -> u16 {
        1
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, filter: Option<&str>, ctx: &Ctx) {
        self.area = area;

        let input = Input::builder()
            .ctx(ctx)
            .buffer_id(self.buffer_id)
            .spacing(1)
            .borderless(true)
            .label(self.label.as_ref())
            .label_style(if self.is_current && !ctx.input.is_active(self.buffer_id) {
                ctx.config.theme.current_item_style
            } else if let Some(f) = filter
                && self.label.to_lowercase().contains(f)
            {
                ctx.config.theme.highlighted_item_style
            } else {
                Style::default()
            })
            .build();

        input.render(area, buf);
    }

    fn left_click(&mut self, pos: Position, ctx: &Ctx) {
        if ctx.input.is_active(self.buffer_id) || !self.area.contains(pos) {
            return;
        }

        self.is_current = true;
    }

    fn double_click(&mut self, pos: Position, ctx: &Ctx) -> Result<bool> {
        if ctx.input.is_active(self.buffer_id) || !self.area.contains(pos) {
            return Ok(false);
        }

        self.is_current = true;
        ctx.input.insert_mode(self.buffer_id);

        Ok(true)
    }

    fn item_labels_iter(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        Box::new(std::iter::once(self.label.as_ref()))
    }
}
