use anyhow::{Context, Result};
use crossterm::event::KeyCode;
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
    shared::{
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
};

use super::{
    dirstack::{DirStack, DirStackItem},
    panes::Pane,
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
    fn next(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()>;
    fn list_songs_in_item(&self, client: &mut impl MpdClient, item: &T) -> Result<Vec<Song>>;
    fn move_selected(&mut self, direction: MoveDirection, client: &mut impl MpdClient) -> Result<()> {
        Ok(())
    }
    fn prepare_preview(
        &mut self,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<Option<Vec<ListItem<'static>>>>;
    fn add(&self, item: &T, client: &mut impl MpdClient, context: &AppContext) -> Result<()>;
    fn add_all(&self, client: &mut impl MpdClient, context: &AppContext) -> Result<()>;
    fn open(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()>;
    fn delete(&self, item: &T, index: usize, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        Ok(())
    }
    fn rename(&self, item: &T, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        Ok(())
    }
    fn handle_filter_input(
        &mut self,
        event: &mut KeyEvent,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<()> {
        if !self.is_filter_input_mode_active() {
            return Ok(());
        }

        let config = context.config;
        match event.as_common_action(context) {
            Some(CommonAction::Close) => {
                self.set_filter_input_mode_active(false);
                self.stack_mut().current_mut().set_filter(None, config);
                let preview = self.prepare_preview(client, config)?;
                self.stack_mut().set_preview(preview);
                context.render()?;
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
                        let preview = self.prepare_preview(client, config)?;
                        self.stack_mut().set_preview(preview);
                        context.render()?;
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

    fn handle_global_action(
        &mut self,
        event: &mut KeyEvent,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<()> {
        let Some(action) = event.as_global_action(context) else {
            return Ok(());
        };

        let config = context.config;
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
            }
            GlobalAction::ExternalCommand { command, .. } => {
                if let Some(selected) = self.stack().current().selected() {
                    let songs = self.list_songs_in_item(client, selected)?;
                    let songs = songs.iter().map(|s| s.file.as_str());

                    run_external(command, create_env(context, songs, client)?);
                }
            }
            _ => {
                event.abandon();
            }
        };

        Ok(())
    }

    fn handle_mouse_action(
        &mut self,
        event: MouseEvent,
        client: &mut impl MpdClient,
        context: &mut AppContext,
    ) -> Result<()> {
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

                context.render()?;
            }
            MouseEventKind::DoubleClick if current_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) = self.stack().current().state.get_at_rendered_row(clicked_row) {
                    self.next(client, context)?;
                    let preview = self
                        .prepare_preview(client, context.config)
                        .context("Cannot prepare preview")?;
                    self.stack_mut().set_preview(preview);

                    context.render()?;
                }
            }
            MouseEventKind::MiddleClick if current_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) = self.stack().current().state.get_at_rendered_row(clicked_row) {
                    self.stack_mut()
                        .current_mut()
                        .select_idx(idx_to_select, context.config.scrolloff);
                    if let Some(item) = self.stack().current().selected() {
                        self.add(item, client, context)?;
                    }

                    let preview = self
                        .prepare_preview(client, context.config)
                        .context("Cannot prepare preview")?;
                    self.stack_mut().set_preview(preview);

                    context.render()?;
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
                    context.render()?;
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

                self.next(client, context)?;
                self.stack_mut()
                    .current_mut()
                    .select_idx(idx_to_select.unwrap_or_default(), 0);

                let preview = self
                    .prepare_preview(client, context.config)
                    .context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                context.render()?;
            }
            MouseEventKind::ScrollUp if current_area.contains(position) => {
                self.stack_mut().current_mut().prev(context.config.scrolloff, false);
                let preview = self
                    .prepare_preview(client, context.config)
                    .context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                context.render()?;
            }
            MouseEventKind::ScrollDown if current_area.contains(position) => {
                self.stack_mut().current_mut().next(context.config.scrolloff, false);
                let preview = self
                    .prepare_preview(client, context.config)
                    .context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                context.render()?;
            }
            _ => {}
        };

        Ok(())
    }

    fn handle_common_action(
        &mut self,
        event: &mut KeyEvent,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<()> {
        let Some(action) = event.as_common_action(context) else {
            return Ok(());
        };
        let config = context.config;

        match action {
            CommonAction::Up => {
                self.stack_mut()
                    .current_mut()
                    .prev(config.scrolloff, config.wrap_navigation);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                context.render()?;
            }
            CommonAction::Down => {
                self.stack_mut()
                    .current_mut()
                    .next(config.scrolloff, config.wrap_navigation);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                context.render()?;
            }
            CommonAction::MoveUp => {
                self.move_selected(MoveDirection::Up, client);
            }
            CommonAction::MoveDown => {
                self.move_selected(MoveDirection::Down, client);
            }
            CommonAction::DownHalf => {
                self.stack_mut()
                    .current_mut()
                    .next_half_viewport(context.config.scrolloff);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                context.render()?;
            }
            CommonAction::UpHalf => {
                self.stack_mut()
                    .current_mut()
                    .prev_half_viewport(context.config.scrolloff);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                context.render()?;
            }
            CommonAction::Bottom => {
                self.stack_mut().current_mut().last();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                context.render()?;
            }
            CommonAction::Top => {
                self.stack_mut().current_mut().first();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                context.render()?;
            }
            CommonAction::Right => {
                self.next(client, context)?;
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);
            }
            CommonAction::Left => {
                self.stack_mut().pop();
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                context.render()?;
            }
            CommonAction::EnterSearch => {
                self.set_filter_input_mode_active(true);
                self.stack_mut().current_mut().set_filter(Some(String::new()), config);

                context.render()?;
            }
            CommonAction::NextResult => {
                self.stack_mut().current_mut().jump_next_matching(config);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                context.render()?;
            }
            CommonAction::PreviousResult => {
                self.stack_mut().current_mut().jump_previous_matching(config);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                context.render()?;
            }
            CommonAction::Select => {
                self.stack_mut().current_mut().toggle_mark_selected();
                self.stack_mut()
                    .current_mut()
                    .next(context.config.scrolloff, context.config.wrap_navigation);
                let preview = self.prepare_preview(client, config).context("Cannot prepare preview")?;
                self.stack_mut().set_preview(preview);

                context.render()?;
            }
            CommonAction::Add if !self.stack().current().marked().is_empty() => {
                for idx in self.stack().current().marked().iter().rev() {
                    let item = &self.stack().current().items[*idx];
                    self.add(item, client, context)?;
                }

                context.render()?;
            }
            CommonAction::Add => {
                if let Some(item) = self.stack().current().selected() {
                    self.add(item, client, context);
                }
            }
            CommonAction::AddAll if !self.stack().current().items.is_empty() => {
                log::debug!("add all");
                self.add_all(client, context)?;

                context.render()?;
            }
            CommonAction::AddAll => {}
            CommonAction::Delete if !self.stack().current().marked().is_empty() => {
                for idx in self.stack().current().marked().iter().rev() {
                    let item = &self.stack().current().items[*idx];
                    self.delete(item, *idx, client, context)?;
                }

                context.render()?;
            }
            CommonAction::Delete => {
                if let Some((index, item)) = self.stack().current().selected_with_idx() {
                    self.delete(item, index, client, context)?;
                    context.render()?;
                }
            }
            CommonAction::Rename => {
                if let Some(item) = self.stack().current().selected() {
                    self.rename(item, client, context);
                }
            }
            CommonAction::FocusInput => {}
            CommonAction::Close => {}
            CommonAction::Confirm if self.stack().current().marked().is_empty() => {
                self.open(client, context)?;
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
