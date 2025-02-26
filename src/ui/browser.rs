use anyhow::Result;
use crossterm::event::KeyCode;
use itertools::Itertools;
use ratatui::prelude::Rect;

use super::{
    dirstack::{DirStack, DirStackItem},
    panes::Pane,
};
use crate::{
    MpdQueryResult,
    config::keys::{CommonAction, GlobalAction},
    context::AppContext,
    mpd::{client::Client, commands::Song},
    shared::{
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
        mpd_query::EXTERNAL_COMMAND,
    },
};

pub enum MoveDirection {
    Up,
    Down,
}

#[allow(unused)]
pub(in crate::ui) trait BrowserPane<T>: Pane
where
    T: DirStackItem + std::fmt::Debug + Clone + Send + 'static,
{
    fn stack(&self) -> &DirStack<T>;
    fn stack_mut(&mut self) -> &mut DirStack<T>;
    fn browser_areas(&self) -> [Rect; 3];
    fn set_filter_input_mode_active(&mut self, active: bool);
    fn is_filter_input_mode_active(&self) -> bool;
    fn next(&mut self, context: &AppContext) -> Result<()>;
    fn list_songs_in_item(
        &self,
        item: T,
    ) -> impl FnOnce(&mut Client<'_>) -> Result<Vec<Song>> + Send + 'static;
    fn prepare_preview(&mut self, context: &AppContext) -> Result<()>;
    fn add(&self, item: &T, context: &AppContext) -> Result<()>;
    fn add_all(&self, context: &AppContext) -> Result<()>;
    fn open(&mut self, context: &AppContext) -> Result<()>;
    fn delete(&self, item: &T, index: usize, context: &AppContext) -> Result<()> {
        Ok(())
    }
    fn rename(&self, item: &T, context: &AppContext) -> Result<()> {
        Ok(())
    }
    fn move_selected(&mut self, direction: MoveDirection, context: &AppContext) -> Result<()> {
        Ok(())
    }
    fn handle_filter_input(&mut self, event: &mut KeyEvent, context: &AppContext) -> Result<()> {
        if !self.is_filter_input_mode_active() {
            return Ok(());
        }

        let config = context.config;
        match event.as_common_action(context) {
            Some(CommonAction::Close) => {
                self.set_filter_input_mode_active(false);
                self.stack_mut().current_mut().set_filter(None, config);
                self.prepare_preview(context);
            }
            Some(CommonAction::Confirm) => {
                self.set_filter_input_mode_active(false);
                context.render()?;
            }
            _ => {
                event.stop_propagation();
                match event.code() {
                    KeyCode::Char(c) => {
                        self.stack_mut().current_mut().push_filter(c, config);
                        self.stack_mut().current_mut().jump_first_matching(config);
                        self.prepare_preview(context);
                    }
                    KeyCode::Backspace => {
                        self.stack_mut().current_mut().pop_filter(config);
                        context.render()?;
                    }
                    _ => {}
                }
            }
        };

        Ok(())
    }

    fn handle_global_action(&mut self, event: &mut KeyEvent, context: &AppContext) -> Result<()> {
        let Some(action) = event.as_global_action(context) else {
            return Ok(());
        };

        let config = context.config;
        match action.clone() {
            GlobalAction::ExternalCommand { command, .. }
                if !self.stack().current().marked().is_empty() =>
            {
                let marked_items: Vec<_> = self
                    .stack()
                    .current()
                    .marked_items()
                    .map(|item| self.list_songs_in_item(item.clone()))
                    .collect();
                let path = self.stack().path().to_owned();
                context.query().id(EXTERNAL_COMMAND).query(move |client| {
                    let songs: Vec<_> = marked_items
                        .into_iter()
                        .map(|item| (item)(client))
                        .flatten_ok()
                        .try_collect()?;
                    Ok(MpdQueryResult::ExternalCommand(command, songs))
                });
            }
            GlobalAction::ExternalCommand { command, .. } => {
                if let Some(selected) = self.stack().current().selected() {
                    let selected = selected.clone();
                    let path = self.stack().path().to_owned();
                    let songs = self.list_songs_in_item(selected);
                    context.query().id(EXTERNAL_COMMAND).query(move |client| {
                        let songs = (songs)(client)?;
                        Ok(MpdQueryResult::ExternalCommand(command, songs))
                    });
                }
            }
            _ => {
                event.abandon();
            }
        };

        Ok(())
    }

    fn handle_mouse_action(&mut self, event: MouseEvent, context: &AppContext) -> Result<()> {
        let [prev_area, current_area, preview_area] = self.browser_areas();

        let position = event.into();
        match event.kind {
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick
                if prev_area.contains(position) =>
            {
                let clicked_row: usize = event.y.saturating_sub(prev_area.y).into();
                let prev_stack = self.stack_mut().previous_mut();
                if let Some(idx_to_select) = prev_stack.state.get_at_rendered_row(clicked_row) {
                    prev_stack.select_idx(idx_to_select, context.config.scrolloff);
                }
                self.stack_mut().pop();
                self.prepare_preview(context);
            }
            MouseEventKind::DoubleClick if current_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) =
                    self.stack().current().state.get_at_rendered_row(clicked_row)
                {
                    self.next(context)?;
                    self.prepare_preview(context);
                }
            }
            MouseEventKind::MiddleClick if current_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) =
                    self.stack().current().state.get_at_rendered_row(clicked_row)
                {
                    self.stack_mut()
                        .current_mut()
                        .select_idx(idx_to_select, context.config.scrolloff);
                    if let Some(item) = self.stack().current().selected() {
                        self.add(item, context)?;
                    }

                    self.prepare_preview(context);
                }
            }
            MouseEventKind::LeftClick if current_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) =
                    self.stack().current().state.get_at_rendered_row(clicked_row)
                {
                    self.stack_mut()
                        .current_mut()
                        .select_idx(idx_to_select, context.config.scrolloff);
                    self.prepare_preview(context);
                }
            }
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick
                if preview_area.contains(position) =>
            {
                let clicked_row: usize = event.y.saturating_sub(preview_area.y).into();
                // Offset does not need to be accounted for since it is always
                // scrolled all the way to the top when going
                // deeper
                let idx_to_select = self.stack().preview().and_then(|preview| {
                    if clicked_row < preview.len() { Some(clicked_row) } else { None }
                });

                self.next(context)?;
                self.stack_mut().current_mut().select_idx(idx_to_select.unwrap_or_default(), 0);

                self.prepare_preview(context);
            }
            MouseEventKind::ScrollUp if current_area.contains(position) => {
                self.stack_mut().current_mut().prev(context.config.scrolloff, false);
                self.prepare_preview(context);
            }
            MouseEventKind::ScrollDown if current_area.contains(position) => {
                self.stack_mut().current_mut().next(context.config.scrolloff, false);
                self.prepare_preview(context);
            }
            _ => {}
        };

        Ok(())
    }

    fn handle_common_action(&mut self, event: &mut KeyEvent, context: &AppContext) -> Result<()> {
        let Some(action) = event.as_common_action(context) else {
            return Ok(());
        };
        let config = context.config;

        match action {
            CommonAction::Up => {
                self.stack_mut().current_mut().prev(config.scrolloff, config.wrap_navigation);
                self.prepare_preview(context);
                context.render()?;
            }
            CommonAction::Down => {
                self.stack_mut().current_mut().next(config.scrolloff, config.wrap_navigation);
                self.prepare_preview(context);
                context.render()?;
            }
            CommonAction::MoveUp => {
                self.move_selected(MoveDirection::Up, context);
            }
            CommonAction::MoveDown => {
                self.move_selected(MoveDirection::Down, context);
            }
            CommonAction::DownHalf => {
                self.stack_mut().current_mut().next_half_viewport(context.config.scrolloff);
                self.prepare_preview(context);
                context.render()?;
            }
            CommonAction::UpHalf => {
                self.stack_mut().current_mut().prev_half_viewport(context.config.scrolloff);
                self.prepare_preview(context);
                context.render()?;
            }
            CommonAction::PageUp => {
                self.stack_mut().current_mut().prev_viewport(context.config.scrolloff);
                self.prepare_preview(context);
                context.render()?;
            }
            CommonAction::PageDown => {
                self.stack_mut().current_mut().next_viewport(context.config.scrolloff);
                self.prepare_preview(context);
                context.render()?;
            }
            CommonAction::Bottom => {
                self.stack_mut().current_mut().last();
                self.prepare_preview(context);
                context.render()?;
            }
            CommonAction::Top => {
                self.stack_mut().current_mut().first();
                self.prepare_preview(context);
                context.render()?;
            }
            CommonAction::Right => {
                self.next(context)?;
                self.prepare_preview(context);
                context.render()?;
            }
            CommonAction::Left => {
                self.stack_mut().pop();
                self.stack_mut().clear_preview();
                self.prepare_preview(context);
                context.render()?;
            }
            CommonAction::EnterSearch => {
                self.set_filter_input_mode_active(true);
                self.stack_mut().current_mut().set_filter(Some(String::new()), config);

                context.render()?;
            }
            CommonAction::NextResult => {
                self.stack_mut().current_mut().jump_next_matching(config);
                self.prepare_preview(context);
                context.render()?;
            }
            CommonAction::PreviousResult => {
                self.stack_mut().current_mut().jump_previous_matching(config);
                self.prepare_preview(context);
                context.render()?;
            }
            CommonAction::InvertSelection => {
                self.stack_mut().current_mut().invert_marked();

                context.render()?;
            }
            CommonAction::Select => {
                self.stack_mut().current_mut().toggle_mark_selected();
                self.stack_mut()
                    .current_mut()
                    .next(context.config.scrolloff, context.config.wrap_navigation);
                self.prepare_preview(context);
                context.render()?;
            }
            CommonAction::Add if !self.stack().current().marked().is_empty() => {
                for idx in self.stack().current().marked() {
                    let item = &self.stack().current().items[*idx];
                    self.add(item, context)?;
                }

                context.render()?;
            }
            CommonAction::Add => {
                if let Some(item) = self.stack().current().selected() {
                    self.add(item, context);
                }
            }
            CommonAction::AddAll if !self.stack().current().items.is_empty() => {
                log::debug!("add all");
                self.add_all(context)?;
            }
            CommonAction::AddAll => {}
            CommonAction::Delete if !self.stack().current().marked().is_empty() => {
                for idx in self.stack().current().marked().iter().rev() {
                    let item = &self.stack().current().items[*idx];
                    self.delete(item, *idx, context)?;
                }

                context.render()?;
            }
            CommonAction::Delete => {
                if let Some((index, item)) = self.stack().current().selected_with_idx() {
                    self.delete(item, index, context)?;
                    context.render()?;
                }
            }
            CommonAction::Rename => {
                if let Some(item) = self.stack().current().selected() {
                    self.rename(item, context);
                }
            }
            CommonAction::FocusInput => {}
            CommonAction::Close => {}
            CommonAction::Confirm if self.stack().current().marked().is_empty() => {
                self.open(context)?;
                context.render()?;
            }
            CommonAction::Confirm => {}
            CommonAction::PaneDown => {}
            CommonAction::PaneUp => {}
            CommonAction::PaneRight => {}
            CommonAction::PaneLeft => {}
        }

        Ok(())
    }
}
