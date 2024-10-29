use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent};
use itertools::Itertools;
use ratatui::{prelude::Rect, widgets::ListItem};

use crate::{
    cli::{create_env, run_external},
    config::{
        keys::{CommonAction, GlobalAction},
        Config,
    },
    context::AppContext,
    mpd::{commands::Song, mpd_client::MpdClient},
    shared::mouse_event::{MouseEvent, MouseEventKind},
};

use super::{
    dirstack::{DirStack, DirStackItem},
    panes::Pane,
    KeyHandleResultInternal,
};

pub enum MoveDirection {
    Up,
    Down,
}

#[allow(unused)]
pub(in crate::ui) trait BrowserPane<T: DirStackItem + std::fmt::Debug>: Pane {
    fn stack(&self) -> &DirStack<T>;
    fn stack_mut(&mut self) -> &mut DirStack<T>;
    fn browser_areas(&self) -> [Rect; 3];
    fn set_filter_input_mode_active(&mut self, active: bool);
    fn is_filter_input_mode_active(&self) -> bool;
    fn next(&mut self, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal>;
    fn list_songs_in_item(&self, client: &mut impl MpdClient, item: &T) -> Result<Vec<Song>>;
    fn move_selected(
        &mut self,
        direction: MoveDirection,
        client: &mut impl MpdClient,
    ) -> Result<KeyHandleResultInternal> {
        Ok(KeyHandleResultInternal::SkipRender)
    }
    fn prepare_preview(
        &mut self,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<Option<Vec<ListItem<'static>>>>;
    fn add(&self, item: &T, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal>;
    fn add_all(&self, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal>;
    fn delete(&self, item: &T, index: usize, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        Ok(KeyHandleResultInternal::SkipRender)
    }
    fn rename(&self, item: &T, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        Ok(KeyHandleResultInternal::SkipRender)
    }
    fn handle_filter_input(
        &mut self,
        event: KeyEvent,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        match config.keybinds.navigation.get(&event.into()) {
            Some(CommonAction::Close) => {
                self.set_filter_input_mode_active(false);
                self.stack_mut().current_mut().set_filter(None, config);
                let preview = self.prepare_preview(client, config)?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            Some(CommonAction::Confirm) => {
                self.set_filter_input_mode_active(false);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            _ => match event.code {
                KeyCode::Char(c) => {
                    self.stack_mut().current_mut().push_filter(c, config);
                    self.stack_mut().current_mut().jump_first_matching(config);
                    let preview = self.prepare_preview(client, config)?;
                    self.stack_mut().set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Backspace => {
                    self.stack_mut().current_mut().pop_filter(config);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                _ => Ok(KeyHandleResultInternal::SkipRender),
            },
        }
    }

    fn handle_global_action(
        &mut self,
        action: GlobalAction,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<KeyHandleResultInternal> {
        match action {
            GlobalAction::ExternalCommand { command, .. } if !self.stack().current().marked().is_empty() => {
                let songs: Vec<_> = self
                    .stack()
                    .current()
                    .marked_items()
                    .map(|item| self.list_songs_in_item(client, item))
                    .flatten_ok()
                    .try_collect()?;
                let songs = songs.iter().map(|song| song.file.as_str()).collect_vec();

                run_external(command, create_env(context, songs, client)?);

                Ok(KeyHandleResultInternal::SkipRender)
            }
            GlobalAction::ExternalCommand { command, .. } => {
                if let Some(selected) = self.stack().current().selected() {
                    let songs = self.list_songs_in_item(client, selected)?;
                    let songs = songs.iter().map(|s| s.file.as_str());

                    run_external(command, create_env(context, songs, client)?);
                }
                Ok(KeyHandleResultInternal::SkipRender)
            }
            _ => Ok(KeyHandleResultInternal::KeyNotHandled),
        }
    }

    fn handle_mouse_action(
        &mut self,
        event: MouseEvent,
        client: &mut impl MpdClient,
        context: &mut AppContext,
    ) -> Result<KeyHandleResultInternal> {
        let [prev_area, current_area, preview_area] = self.browser_areas();

        let position = event.into();
        match event.kind {
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick if prev_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(prev_area.y).into();
                let prev_stack = self.stack_mut().previous_mut();
                if let Some(idx_to_select) = prev_stack.state.get_at_rendered_row(clicked_row) {
                    prev_stack.select_idx(idx_to_select, context.config.scrolloff);
                }
                self.stack_mut().pop();
                let preview = self
                    .prepare_preview(client, context.config)
                    .context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                Ok(KeyHandleResultInternal::RenderRequested)
            }
            MouseEventKind::DoubleClick if current_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) = self.stack().current().state.get_at_rendered_row(clicked_row) {
                    let res = self.next(client)?;
                    let preview = self
                        .prepare_preview(client, context.config)
                        .context("Cannot prepare preview")?;
                    self.stack_mut().set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                } else {
                    Ok(KeyHandleResultInternal::SkipRender)
                }
            }
            MouseEventKind::MiddleClick if current_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) = self.stack().current().state.get_at_rendered_row(clicked_row) {
                    self.stack_mut()
                        .current_mut()
                        .select_idx(idx_to_select, context.config.scrolloff);
                    if let Some(item) = self.stack().current().selected() {
                        self.add(item, client)?;
                    }

                    let preview = self
                        .prepare_preview(client, context.config)
                        .context("Cannot prepare preview")?;
                    self.stack_mut().set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                } else {
                    Ok(KeyHandleResultInternal::SkipRender)
                }
            }
            MouseEventKind::LeftClick if current_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) = self.stack().current().state.get_at_rendered_row(clicked_row) {
                    self.stack_mut()
                        .current_mut()
                        .select_idx(idx_to_select, context.config.scrolloff);
                    let preview = self
                        .prepare_preview(client, context.config)
                        .context("Cannot prepare preview")?;
                    self.stack_mut().set_preview(preview);
                    Ok(KeyHandleResultInternal::RenderRequested)
                } else {
                    Ok(KeyHandleResultInternal::SkipRender)
                }
            }
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick if preview_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(preview_area.y).into();
                // Offset does not need to be accounted for since it is always scrolled all the way
                // to the top when going deeper
                let idx_to_select = self.stack().preview().and_then(|preview| {
                    if clicked_row < preview.len() {
                        Some(clicked_row)
                    } else {
                        None
                    }
                });

                let res = self.next(client)?;
                self.stack_mut()
                    .current_mut()
                    .select_idx(idx_to_select.unwrap_or_default(), 0);

                let preview = self
                    .prepare_preview(client, context.config)
                    .context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                Ok(KeyHandleResultInternal::RenderRequested)
            }
            MouseEventKind::ScrollUp if current_area.contains(position) => {
                self.stack_mut().current_mut().prev(context.config.scrolloff, false);
                let preview = self
                    .prepare_preview(client, context.config)
                    .context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                Ok(KeyHandleResultInternal::RenderRequested)
            }
            MouseEventKind::ScrollDown if current_area.contains(position) => {
                self.stack_mut().current_mut().next(context.config.scrolloff, false);
                let preview = self
                    .prepare_preview(client, context.config)
                    .context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                Ok(KeyHandleResultInternal::RenderRequested)
            }
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }

    fn handle_common_action(
        &mut self,
        action: CommonAction,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<KeyHandleResultInternal> {
        let config = context.config;
        match action {
            CommonAction::Up => {
                self.stack_mut()
                    .current_mut()
                    .prev(config.scrolloff, config.wrap_navigation);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Down => {
                self.stack_mut()
                    .current_mut()
                    .next(config.scrolloff, config.wrap_navigation);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::MoveUp => {
                let res = self.move_selected(MoveDirection::Up, client)?;
                Ok(res)
            }
            CommonAction::MoveDown => {
                let res = self.move_selected(MoveDirection::Down, client)?;
                Ok(res)
            }
            CommonAction::DownHalf => {
                self.stack_mut()
                    .current_mut()
                    .next_half_viewport(context.config.scrolloff);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::UpHalf => {
                self.stack_mut()
                    .current_mut()
                    .prev_half_viewport(context.config.scrolloff);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Bottom => {
                self.stack_mut().current_mut().last();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Top => {
                self.stack_mut().current_mut().first();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Right => {
                let res = self.next(client)?;
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(res)
            }
            CommonAction::Left => {
                self.stack_mut().pop();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::EnterSearch => {
                self.set_filter_input_mode_active(true);
                self.stack_mut().current_mut().set_filter(Some(String::new()), config);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::NextResult => {
                self.stack_mut().current_mut().jump_next_matching(config);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::PreviousResult => {
                self.stack_mut().current_mut().jump_previous_matching(config);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Select => {
                self.stack_mut().current_mut().toggle_mark_selected();
                self.stack_mut()
                    .current_mut()
                    .next(context.config.scrolloff, context.config.wrap_navigation);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Add if !self.stack().current().marked().is_empty() => {
                for idx in self.stack().current().marked().iter().rev() {
                    let item = &self.stack().current().items[*idx];
                    self.add(item, client)?;
                }
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Add => {
                if let Some(item) = self.stack().current().selected() {
                    self.add(item, client)
                } else {
                    Ok(KeyHandleResultInternal::SkipRender)
                }
            }
            CommonAction::AddAll if !self.stack().current().items.is_empty() => {
                self.add_all(client)?;
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::AddAll => Ok(KeyHandleResultInternal::SkipRender),
            CommonAction::Delete if !self.stack().current().marked().is_empty() => {
                for idx in self.stack().current().marked().iter().rev() {
                    let item = &self.stack().current().items[*idx];
                    self.delete(item, *idx, client)?;
                }
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            CommonAction::Delete => {
                if let Some((index, item)) = self.stack().current().selected_with_idx() {
                    self.delete(item, index, client)?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                } else {
                    Ok(KeyHandleResultInternal::SkipRender)
                }
            }
            CommonAction::Rename => {
                if let Some(item) = self.stack().current().selected() {
                    self.rename(item, client)
                } else {
                    Ok(KeyHandleResultInternal::SkipRender)
                }
            }
            CommonAction::FocusInput => Ok(KeyHandleResultInternal::SkipRender),
            CommonAction::Close => Ok(KeyHandleResultInternal::SkipRender), // todo out?
            CommonAction::Confirm => Ok(KeyHandleResultInternal::SkipRender), // todo next?
            CommonAction::PaneDown => Ok(KeyHandleResultInternal::SkipRender),
            CommonAction::PaneUp => Ok(KeyHandleResultInternal::SkipRender),
            CommonAction::PaneRight => Ok(KeyHandleResultInternal::SkipRender),
            CommonAction::PaneLeft => Ok(KeyHandleResultInternal::SkipRender),
        }
    }
}
