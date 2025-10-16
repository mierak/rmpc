use std::borrow::Cow;

use anyhow::Result;
use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    widgets::Widget,
};

use super::Section;
use crate::{ctx::Ctx, shared::key_event::KeyEvent, ui::widgets::input::Input};

#[derive(derive_more::Debug, Default)]
pub struct InputSection<'a> {
    pub value: String,
    pub label: Cow<'a, str>,
    pub area: Rect,
    pub current_item_style: Style,
    pub is_current: bool,
    pub is_focused: bool,
    #[debug(skip)]
    pub action: Option<Box<dyn FnOnce(&Ctx, String) + Send + Sync + 'static>>,
}

impl<'a> InputSection<'a> {
    pub fn new(label: impl Into<Cow<'a, str>>, current_item_style: Style) -> Self {
        Self {
            area: Rect::default(),
            current_item_style,
            value: String::new(),
            label: label.into(),
            action: None,
            is_current: false,
            is_focused: false,
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

    pub fn add_initial_value(&mut self, value: impl Into<String>) -> &mut Self {
        self.value = value.into();
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

    fn unfocus(&mut self) {
        self.is_focused = false;
    }

    fn unselect(&mut self) {
        self.is_focused = false;
        self.is_current = false;
    }

    fn confirm(&mut self, ctx: &Ctx) -> Result<bool> {
        if self.is_focused {
            if let Some(cb) = self.action.take() {
                (cb)(ctx, std::mem::take(&mut self.value));
            }
            Ok(false)
        } else {
            self.is_focused = true;
            Ok(true)
        }
    }

    fn len(&self) -> usize {
        1
    }

    fn preferred_height(&self) -> u16 {
        1
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _ctx: &Ctx) {
        self.area = area;

        let input = Input::default()
            .set_label_style(if self.is_current {
                self.current_item_style
            } else {
                Style::default()
            })
            .spacing(1)
            .set_borderless(true)
            .set_label(self.label.as_ref())
            .set_focused(self.is_focused)
            .set_text(&self.value);

        input.render(area, buf);
    }

    fn left_click(&mut self, pos: Position) {
        if self.is_focused || !self.area.contains(pos) {
            return;
        }

        self.is_current = true;
    }

    fn double_click(&mut self, pos: Position, _ctx: &Ctx) -> Result<bool> {
        if self.is_focused || !self.area.contains(pos) {
            return Ok(false);
        }

        self.is_current = true;
        self.is_focused = true;

        Ok(true)
    }

    fn key_input(&mut self, key: &mut KeyEvent, ctx: &Ctx) -> Result<()> {
        match key.code() {
            KeyCode::Char(c) => {
                self.value.push(c);

                ctx.render()?;
            }
            KeyCode::Backspace => {
                self.value.pop();

                ctx.render()?;
            }
            _ => {}
        }

        Ok(())
    }
}
