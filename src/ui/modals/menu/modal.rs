use std::borrow::Cow;

use anyhow::Result;
use crossterm::event::KeyCode;
use itertools::Itertools;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::Style,
    symbols::border,
    widgets::{Block, Borders, Clear},
};

use super::{
    Section,
    SectionType,
    input_section::InputSection,
    list_section::ListSection,
    multi_action_section::MultiActionSection,
};
use crate::{
    config::keys::CommonAction,
    ctx::Ctx,
    shared::{
        id::{self, Id},
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::modals::{Modal, RectExt as _, menu::select_section::SelectSection},
};

#[derive(Debug)]
pub struct MenuModal<'a> {
    sections: Vec<SectionType<'a>>,
    sections_labels: Vec<Vec<String>>,
    current_section_idx: usize,
    areas: Vec<Rect>,
    input_focused: bool,
    width: u16,
    id: Id,
    filter: Option<String>,
    input_mode: bool,
}

impl Modal for MenuModal<'_> {
    fn id(&self) -> Id {
        self.id
    }

    fn render(&mut self, frame: &mut Frame, ctx: &mut Ctx) -> Result<()> {
        let needed_height: usize =
            self.sections.iter().map(|section| section.preferred_height() as usize).sum::<usize>()
                + 1
                + self.sections.len();

        let popup_area = frame.area().centered_exact(self.width, needed_height as u16);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = ctx.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(ctx.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center);
        if let Some(filter) = self.filter.as_ref() {
            block = block.title(format!(" [FILTER]: {filter} "));
        }

        let content_area = block.inner(popup_area);

        let areas = Layout::vertical(Itertools::intersperse(
            self.sections.iter_mut().map(|s| Constraint::Length(s.preferred_height())),
            Constraint::Length(1),
        ))
        .split(content_area);

        let mut section_idx = 0;
        for (idx, area) in areas.iter().enumerate() {
            if idx % 2 == 0 {
                self.sections[section_idx].render(
                    *area,
                    frame.buffer_mut(),
                    self.filter.as_deref(),
                    ctx,
                );
                self.areas[section_idx] = *area;
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

    fn handle_key(&mut self, key: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        if self.input_focused {
            let action = key.as_common_action(ctx);
            if let Some(CommonAction::Close) = action {
                self.input_focused = false;
                self.sections[self.current_section_idx].unfocus();

                ctx.render()?;
                return Ok(());
            } else if let Some(CommonAction::Confirm) = action {
                self.sections[self.current_section_idx].confirm(ctx)?;

                self.hide(ctx)?;
                return Ok(());
            }

            self.sections[self.current_section_idx].key_input(key, ctx)?;

            return Ok(());
        }

        if let Some(filter) = &mut self.filter
            && self.input_mode
        {
            match key.as_common_action(ctx) {
                Some(CommonAction::Close) => {
                    self.input_mode = false;
                    self.filter = None;
                    ctx.render()?;
                }
                Some(CommonAction::Confirm) => {
                    self.input_mode = false;
                    ctx.render()?;
                }
                _ => {
                    key.stop_propagation();
                    match key.code() {
                        KeyCode::Char(c) => {
                            filter.push(c);
                            self.first_result();
                            ctx.render()?;
                        }
                        KeyCode::Backspace => {
                            filter.pop();
                            ctx.render()?;
                        }
                        _ => {}
                    }
                }
            }
            return Ok(());
        }

        if let Some(action) = key.as_common_action(ctx) {
            match action {
                CommonAction::EnterSearch => {
                    self.filter = Some(String::new());
                    self.input_mode = true;
                    ctx.render()?;
                }
                CommonAction::Up => {
                    self.prev();
                    ctx.render()?;
                }
                CommonAction::Down => {
                    self.next();
                    ctx.render()?;
                }
                CommonAction::Right => {
                    self.sections[self.current_section_idx].right();
                    ctx.render()?;
                }
                CommonAction::Left => {
                    self.sections[self.current_section_idx].left();
                    ctx.render()?;
                }
                CommonAction::Top => {
                    if self.current_section_idx != 0 {
                        self.sections[self.current_section_idx].unselect();
                    }
                    self.current_section_idx = 0;
                    self.sections[0].select(0);
                    ctx.render()?;
                }
                CommonAction::Bottom => {
                    let sect_idx = self.sections.len() - 1;
                    let last_sect_item_idx = self.sections[sect_idx].len() - 1;

                    if self.current_section_idx != sect_idx {
                        self.sections[self.current_section_idx].unselect();
                    }
                    self.current_section_idx = sect_idx;
                    self.sections[sect_idx].select(last_sect_item_idx);
                    ctx.render()?;
                }
                CommonAction::Close => {
                    self.hide(ctx)?;
                }
                CommonAction::Confirm => {
                    self.input_focused = self.sections[self.current_section_idx].confirm(ctx)?;
                    if self.input_focused {
                        ctx.render()?;
                    } else {
                        self.hide(ctx)?;
                    }
                }
                CommonAction::NextResult => {
                    self.next_result();
                    ctx.render()?;
                }
                CommonAction::PreviousResult => {
                    self.prev_result();
                    ctx.render()?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &mut Ctx) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick => {
                if let Some(idx) = self.section_idx_at_position(event.into()) {
                    if idx != self.current_section_idx {
                        self.sections[self.current_section_idx].unselect();
                    }
                    self.current_section_idx = idx;
                    self.sections[idx].left_click(event.into());
                    ctx.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                if let Some(idx) = self.section_idx_at_position(event.into()) {
                    self.input_focused = self.sections[idx].double_click(event.into(), ctx)?;
                    if self.input_focused {
                        ctx.render()?;
                    } else {
                        self.hide(ctx)?;
                    }
                }
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollUp => {
                self.prev();
                ctx.render()?;
            }
            MouseEventKind::ScrollDown => {
                self.next();
                ctx.render()?;
            }
            MouseEventKind::Drag { drag_start_position: _ } => {}
        }
        Ok(())
    }
}

impl<'a> MenuModal<'a> {
    pub fn new(_ctx: &Ctx) -> Self {
        Self {
            sections: Vec::default(),
            sections_labels: Vec::default(),
            current_section_idx: 0,
            areas: Vec::new(),
            input_focused: false,
            width: 40,
            id: id::new(),
            filter: None,
            input_mode: false,
        }
    }

    fn next_result(&mut self) {
        let Some(filter) = self.filter.as_ref() else {
            return;
        };

        let sect_count = self.sections.len();
        let curr_sect_idx = self.current_section_idx;
        for i in curr_sect_idx..sect_count + curr_sect_idx {
            let sect_i = i % sect_count;

            let sect = &self.sections[sect_i];
            let start = sect.selected().map_or(0, |s| s + 1);

            for label_idx in start..sect.len() {
                let label = &self.sections_labels[sect_i][label_idx];
                if label.contains(filter) {
                    if sect_i != self.current_section_idx {
                        self.sections[self.current_section_idx].unselect();
                    }
                    self.current_section_idx = sect_i;
                    self.sections[sect_i].select(label_idx);
                    return;
                }
            }
        }

        // if nothing was found, try to search the current section again from
        // the start to wrap around inside just the section itself
        let sect = &self.sections[self.current_section_idx];
        for label_idx in 0..sect.len() {
            let label = &self.sections_labels[self.current_section_idx][label_idx];
            if label.contains(filter) {
                self.sections[self.current_section_idx].select(label_idx);
                break;
            }
        }
    }

    fn prev_result(&mut self) {
        let Some(filter) = self.filter.as_ref() else {
            return;
        };

        let sect_count = self.sections.len();
        let curr_sect_idx = self.current_section_idx;
        for i in (0..=sect_count).rev() {
            let sect_i = (i + curr_sect_idx) % sect_count;

            let sect = &self.sections[sect_i];
            let end = sect.selected().unwrap_or(sect.len());

            for label_idx in (0..end).rev() {
                let label = &self.sections_labels[sect_i][label_idx];
                if label.contains(filter) {
                    if sect_i != self.current_section_idx {
                        self.sections[self.current_section_idx].unselect();
                    }
                    self.current_section_idx = sect_i;
                    self.sections[sect_i].select(label_idx);
                    return;
                }
            }
        }

        // if nothing was found, try to search the current section again from
        // the end to wrap around inside just the section itself
        let sect = &self.sections[self.current_section_idx];
        for label_idx in (0..sect.len()).rev() {
            let label = &self.sections_labels[self.current_section_idx][label_idx];
            if label.contains(filter) {
                self.sections[self.current_section_idx].select(label_idx);
                break;
            }
        }
    }

    fn first_result(&mut self) {
        let Some(filter) = self.filter.as_ref() else {
            return;
        };

        for sect_i in 0..self.sections_labels.len() {
            for label_idx in 0..self.sections_labels[sect_i].len() {
                let label = &self.sections_labels[sect_i][label_idx];
                if label.contains(filter) {
                    if sect_i != self.current_section_idx {
                        self.sections[self.current_section_idx].unselect();
                    }
                    self.current_section_idx = sect_i;
                    self.sections[sect_i].select(label_idx);
                    return;
                }
            }
        }
    }

    pub fn width(mut self, width: u16) -> Self {
        self.width = width;
        self
    }

    pub fn build(mut self) -> Self {
        if let Some((i, s)) =
            self.sections.iter_mut().enumerate().find_or_first(|(_, s)| s.len() > 0)
        {
            self.current_section_idx = i;
            s.down();
        }
        self.sections_labels =
            self.sections.iter().fold(Vec::<Vec<String>>::new(), |mut acc, s| {
                acc.push(s.item_labels_iter().map(|l| l.to_lowercase()).collect());
                acc
            });
        self
    }

    pub fn list_section(
        mut self,
        ctx: &Ctx,
        cb: impl FnOnce(ListSection) -> Option<ListSection>,
    ) -> Self {
        let section = ListSection::new(ctx.config.theme.current_item_style);
        let section = cb(section);
        if let Some(section) = section {
            self.sections.push(SectionType::Menu(section));
            self.areas.push(Rect::default());
        }
        self
    }

    pub fn multi_section(
        mut self,
        ctx: &Ctx,
        cb: impl FnOnce(MultiActionSection) -> Option<MultiActionSection<'_>>,
    ) -> Self {
        let section = MultiActionSection::new(ctx.config.theme.current_item_style);
        let section = cb(section);
        if let Some(mut section) = section {
            section.build();
            self.sections.push(SectionType::Multi(section));
            self.areas.push(Rect::default());
        }
        self
    }

    pub fn input_section(
        mut self,
        ctx: &Ctx,
        label: impl Into<Cow<'a, str>>,
        cb: impl FnOnce(InputSection) -> Option<InputSection<'_>>,
    ) -> Self {
        let section = InputSection::new(label, ctx.config.theme.current_item_style);
        let section = cb(section);
        if let Some(section) = section {
            self.sections.push(SectionType::Input(section));
            self.areas.push(Rect::default());
        }
        self
    }

    pub fn select_section(
        mut self,
        ctx: &Ctx,
        cb: impl FnOnce(SelectSection) -> Option<SelectSection>,
    ) -> Self {
        let section = SelectSection::new(ctx.config.theme.current_item_style);
        let section = cb(section);
        if let Some(section) = section {
            self.sections.push(SectionType::Select(section));
            self.areas.push(Rect::default());
        }
        self
    }

    fn next(&mut self) {
        let result = self.sections[self.current_section_idx].down();
        if !result {
            self.current_section_idx = (self.current_section_idx + 1) % self.sections.len();
            self.sections[self.current_section_idx].down();
        }
    }

    fn prev(&mut self) {
        let result = self.sections[self.current_section_idx].up();
        if !result {
            self.current_section_idx =
                (self.current_section_idx + self.sections.len() - 1) % self.sections.len();
            self.sections[self.current_section_idx].up();
        }
    }

    fn section_idx_at_position(&self, position: Position) -> Option<usize> {
        self.areas.iter().enumerate().find(|(_, a)| a.contains(position)).map(|(i, _)| i)
    }
}
