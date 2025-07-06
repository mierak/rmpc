#![allow(clippy::cast_possible_truncation)]

use anyhow::Result;
use itertools::Itertools;
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Position, Rect},
    style::Style,
    symbols::border,
    text::Text,
    widgets::{Block, Borders, Clear, Widget},
};

use super::{Modal, RectExt as _};
use crate::{
    config::keys::{CommonAction, actions::AddOpts},
    context::Ctx,
    shared::{
        ext::{
            mpd_client::{Enqueue, MpdClientExt},
            rect::RectExt,
        },
        key_event::KeyEvent,
        macros::pop_modal,
        mouse_event::{MouseEvent, MouseEventKind},
    },
};

#[derive(Debug)]
pub struct MenuModal {
    sections: Vec<MenuSection>,
    current_section_idx: usize,
    area: Rect,
}

#[derive(Debug, Default)]
pub struct MenuSection {
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

impl Modal for MenuModal {
    fn render(&mut self, frame: &mut Frame, ctx: &mut Ctx) -> Result<()> {
        let needed_height: usize = self.sections.iter().map(|section| section.len()).sum::<usize>()
            + 1
            + self.sections.len();

        let popup_area = frame.area().centered_exact(40, needed_height as u16);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = ctx.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }
        self.area = popup_area;

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(ctx.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center);

        let content_area = block.inner(popup_area);

        let areas = Layout::vertical(Itertools::intersperse(
            self.sections.iter().map(|s| Constraint::Length(s.len() as u16)),
            Constraint::Length(1),
        ))
        .split(content_area);

        let mut section_idx = 0;
        for (idx, area) in areas.iter().enumerate() {
            if idx % 2 == 0 {
                self.sections[section_idx].render(*area, frame.buffer_mut());
                section_idx += 1;
            } else {
                let buf = frame.buffer_mut();
                for x in area.left()..area.right() {
                    buf[(x, area.y)]
                        .set_symbol(ratatui::symbols::border::ROUNDED.horizontal_bottom)
                        .set_style(ctx.config.as_border_style());
                }
            }
        }

        frame.render_widget(block, popup_area);

        Ok(())
    }

    fn handle_key(&mut self, key: &mut KeyEvent, context: &mut Ctx) -> Result<()> {
        if let Some(action) = key.as_common_action(context) {
            match action {
                CommonAction::Up => {
                    self.prev();
                    context.render()?;
                }
                CommonAction::Down => {
                    self.next();
                    context.render()?;
                }
                CommonAction::Close => {
                    pop_modal!(context);
                }
                CommonAction::Confirm => {
                    let current_section = &mut self.sections[self.current_section_idx];
                    let item =
                        &mut current_section.items[current_section.selected_idx.unwrap_or(0)];

                    if let Some(cb) = item.on_confirm.take() {
                        (cb)(context);
                    }

                    pop_modal!(context);
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &mut Ctx) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick => {
                self.select_item_at_position(event.into());
                context.render()?;
            }
            MouseEventKind::DoubleClick => {
                if let Some(item) = self.item_at_position(event.into()) {
                    if let Some(cb) = item.on_confirm.take() {
                        (cb)(context);
                    }
                    pop_modal!(context);
                }
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollUp => {
                self.prev();
                context.render()?;
            }
            MouseEventKind::ScrollDown => {
                self.next();
                context.render()?;
            }
        }
        Ok(())
    }
}

impl MenuModal {
    pub fn new(_context: &Ctx) -> Self {
        Self { sections: Vec::default(), current_section_idx: 0, area: Rect::default() }
    }

    pub fn build(mut self) -> Self {
        if let Some(s) = self.sections.get_mut(0) {
            s.selected_idx = Some(0);
        }
        self
    }

    pub fn add_section(
        mut self,
        context: &Ctx,
        cb: impl FnOnce(MenuSection) -> MenuSection,
    ) -> Self {
        let section = MenuSection::new(context.config.theme.current_item_style);
        let section = cb(section);
        self.sections.push(section);
        self
    }

    fn item_at_position(&mut self, position: Position) -> Option<&mut MenuItem> {
        if !self.area.contains(position) {
            return None;
        }

        self.sections.iter_mut().find_map(|s| s.item_at_position(position))
    }

    fn select_item_at_position(&mut self, position: Position) {
        if !self.area.contains(position) {
            return;
        }

        for section in &mut self.sections {
            if section.area.contains(position) {
                section.select_item_at_position(position);
            } else {
                section.selected_idx = None;
            }
        }
    }

    fn next(&mut self) {
        let current_section = &mut self.sections[self.current_section_idx];
        if current_section.selected_idx.is_some_and(|i| i == current_section.len() - 1) {
            current_section.selected_idx = None;
            self.current_section_idx = (self.current_section_idx + 1) % self.sections.len();
            self.sections[self.current_section_idx].selected_idx = Some(0);
        } else {
            current_section.selected_idx =
                current_section.selected_idx.map_or(Some(0), |i| Some(i + 1));
        }
    }

    fn prev(&mut self) {
        let current_section = &mut self.sections[self.current_section_idx];
        if current_section.selected_idx.is_some_and(|i| i == 0) {
            current_section.selected_idx = None;
            self.current_section_idx = if self.current_section_idx == 0 {
                self.sections.len() - 1
            } else {
                self.current_section_idx - 1
            };
            let new_current_section = &mut self.sections[self.current_section_idx];
            new_current_section.selected_idx = Some(new_current_section.len() - 1);
        } else {
            current_section.selected_idx =
                current_section.selected_idx.map_or(Some(0), |i| Some(i.saturating_sub(1)));
        }
    }

    pub fn create_add_modal(
        opts: Vec<(String, AddOpts, (Vec<Enqueue>, Option<usize>))>,
        ctx: &Ctx,
    ) -> MenuModal {
        MenuModal::new(ctx)
            .add_section(ctx, |section| {
                let queue_len = ctx.queue.len();
                let current_song_idx = ctx.find_current_song_in_queue().map(|(i, _)| i);
                let mut section = section;

                for (label, options, (enqueue, hovered_idx)) in opts {
                    section = section.add_item(label, move |ctx| {
                        if !enqueue.is_empty() {
                            ctx.command(move |client| {
                                let autoplay =
                                    options.autoplay(queue_len, current_song_idx, hovered_idx);
                                client.enqueue_multiple(enqueue, options.position, autoplay)?;

                                Ok(())
                            });
                        }
                    });
                }
                section
            })
            .add_section(ctx, |section| section.add_item("Cancel", |_ctx| {}))
            .build()
    }
}

impl MenuSection {
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

    fn len(&self) -> usize {
        self.items.len()
    }

    fn item_at_position(&mut self, position: Position) -> Option<&mut MenuItem> {
        if !self.area.contains(position) {
            return None;
        }

        let idx = position.y.saturating_sub(self.area.y) as usize;
        self.items.get_mut(idx)
    }

    fn select_item_at_position(&mut self, position: Position) {
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

impl Widget for &mut MenuSection {
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
