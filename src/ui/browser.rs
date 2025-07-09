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
    config::keys::{
        CommonAction,
        GlobalAction,
        actions::{AddKind, Position},
    },
    ctx::Ctx,
    mpd::{client::Client, commands::Song},
    shared::{
        ext::mpd_client::{Autoplay, Enqueue, MpdClientExt},
        key_event::KeyEvent,
        macros::modal,
        mouse_event::{MouseEvent, MouseEventKind},
        mpd_query::EXTERNAL_COMMAND,
    },
    ui::modals::menu::create_add_modal,
};

#[derive(Debug, Clone, Copy)]
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
    fn next(&mut self, ctx: &Ctx) -> Result<()>;
    fn list_songs_in_item(
        &self,
        item: T,
    ) -> impl FnOnce(&mut Client<'_>) -> Result<Vec<Song>> + Send + 'static;
    fn prepare_preview(&mut self, ctx: &Ctx) -> Result<()>;
    fn enqueue<'a>(&self, items: impl Iterator<Item = &'a T>) -> (Vec<Enqueue>, Option<usize>);
    fn open(&mut self, ctx: &Ctx) -> Result<()>;
    fn show_info(&self, item: &T, ctx: &Ctx) -> Result<()> {
        Ok(())
    }
    fn delete(&self, item: &T, index: usize, ctx: &Ctx) -> Result<()> {
        Ok(())
    }
    fn rename(&self, item: &T, ctx: &Ctx) -> Result<()> {
        Ok(())
    }
    fn move_selected(&mut self, direction: MoveDirection, ctx: &Ctx) -> Result<()> {
        Ok(())
    }
    fn handle_filter_input(&mut self, event: &mut KeyEvent, ctx: &Ctx) -> Result<()> {
        if !self.is_filter_input_mode_active() {
            return Ok(());
        }

        let config = &ctx.config;
        match event.as_common_action(ctx) {
            Some(CommonAction::Close) => {
                self.set_filter_input_mode_active(false);
                self.stack_mut().current_mut().set_filter(None, config);
                self.prepare_preview(ctx);
            }
            Some(CommonAction::Confirm) => {
                self.set_filter_input_mode_active(false);
                ctx.render()?;
            }
            _ => {
                event.stop_propagation();
                match event.code() {
                    KeyCode::Char(c) => {
                        self.stack_mut().current_mut().push_filter(c, config);
                        self.stack_mut().current_mut().jump_first_matching(config);
                        self.prepare_preview(ctx);
                    }
                    KeyCode::Backspace => {
                        self.stack_mut().current_mut().pop_filter(config);
                        ctx.render()?;
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn handle_global_action(&mut self, event: &mut KeyEvent, ctx: &Ctx) -> Result<()> {
        let Some(action) = event.as_global_action(ctx) else {
            return Ok(());
        };

        let config = &ctx.config;
        match &action {
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
                let command = std::sync::Arc::clone(command);
                ctx.query().id(EXTERNAL_COMMAND).query(move |client| {
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
                    let command = std::sync::Arc::clone(command);
                    ctx.query().id(EXTERNAL_COMMAND).query(move |client| {
                        let songs = (songs)(client)?;
                        Ok(MpdQueryResult::ExternalCommand(command, songs))
                    });
                }
            }
            _ => {
                event.abandon();
            }
        }

        Ok(())
    }

    fn handle_mouse_action(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        let [prev_area, current_area, preview_area] = self.browser_areas();

        let position = event.into();
        match event.kind {
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick
                if prev_area.contains(position) =>
            {
                let clicked_row: usize = event.y.saturating_sub(prev_area.y).into();
                let prev_stack = self.stack_mut().previous_mut();
                if let Some(idx_to_select) = prev_stack.state.get_at_rendered_row(clicked_row) {
                    prev_stack.select_idx(idx_to_select, ctx.config.scrolloff);
                }
                self.stack_mut().pop();
                self.prepare_preview(ctx);
            }
            MouseEventKind::DoubleClick if current_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) =
                    self.stack().current().state.get_at_rendered_row(clicked_row)
                {
                    self.next(ctx)?;
                    self.prepare_preview(ctx);
                }
            }
            MouseEventKind::MiddleClick if current_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) =
                    self.stack().current().state.get_at_rendered_row(clicked_row)
                {
                    self.stack_mut().current_mut().select_idx(idx_to_select, ctx.config.scrolloff);
                    if let Some(item) = self.stack().current().selected() {
                        let (items, _) = self.enqueue(std::iter::once(item));
                        if !items.is_empty() {
                            ctx.command(move |client| {
                                client.enqueue_multiple(
                                    items,
                                    Position::EndOfQueue,
                                    Autoplay::None,
                                )?;
                                Ok(())
                            });
                        }
                    }

                    self.prepare_preview(ctx);
                }
            }
            MouseEventKind::LeftClick if current_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) =
                    self.stack().current().state.get_at_rendered_row(clicked_row)
                {
                    self.stack_mut().current_mut().select_idx(idx_to_select, ctx.config.scrolloff);
                    self.prepare_preview(ctx);
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

                self.next(ctx)?;
                self.stack_mut().current_mut().select_idx(idx_to_select.unwrap_or_default(), 0);

                self.prepare_preview(ctx);
            }
            MouseEventKind::ScrollUp if current_area.contains(position) => {
                self.stack_mut().current_mut().prev(ctx.config.scrolloff, false);
                self.prepare_preview(ctx);
            }
            MouseEventKind::ScrollDown if current_area.contains(position) => {
                self.stack_mut().current_mut().next(ctx.config.scrolloff, false);
                self.prepare_preview(ctx);
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_common_action(&mut self, event: &mut KeyEvent, ctx: &Ctx) -> Result<()> {
        let Some(action) = event.as_common_action(ctx) else {
            return Ok(());
        };
        let config = &ctx.config;

        match action.to_owned() {
            CommonAction::Up => {
                self.stack_mut().current_mut().prev(config.scrolloff, config.wrap_navigation);
                self.prepare_preview(ctx);
                ctx.render()?;
            }
            CommonAction::Down => {
                self.stack_mut().current_mut().next(config.scrolloff, config.wrap_navigation);
                self.prepare_preview(ctx);
                ctx.render()?;
            }
            CommonAction::MoveUp => {
                self.move_selected(MoveDirection::Up, ctx);
            }
            CommonAction::MoveDown => {
                self.move_selected(MoveDirection::Down, ctx);
            }
            CommonAction::DownHalf => {
                self.stack_mut().current_mut().next_half_viewport(ctx.config.scrolloff);
                self.prepare_preview(ctx);
                ctx.render()?;
            }
            CommonAction::UpHalf => {
                self.stack_mut().current_mut().prev_half_viewport(ctx.config.scrolloff);
                self.prepare_preview(ctx);
                ctx.render()?;
            }
            CommonAction::PageUp => {
                self.stack_mut().current_mut().prev_viewport(ctx.config.scrolloff);
                self.prepare_preview(ctx);
                ctx.render()?;
            }
            CommonAction::PageDown => {
                self.stack_mut().current_mut().next_viewport(ctx.config.scrolloff);
                self.prepare_preview(ctx);
                ctx.render()?;
            }
            CommonAction::Bottom => {
                self.stack_mut().current_mut().last();
                self.prepare_preview(ctx);
                ctx.render()?;
            }
            CommonAction::Top => {
                self.stack_mut().current_mut().first();
                self.prepare_preview(ctx);
                ctx.render()?;
            }
            CommonAction::Right => {
                self.next(ctx)?;
                self.prepare_preview(ctx);
                ctx.render()?;
            }
            CommonAction::Left => {
                self.stack_mut().pop();
                self.stack_mut().clear_preview();
                self.prepare_preview(ctx);
                ctx.render()?;
            }
            CommonAction::EnterSearch => {
                self.set_filter_input_mode_active(true);
                self.stack_mut().current_mut().set_filter(Some(String::new()), config);

                ctx.render()?;
            }
            CommonAction::NextResult => {
                self.stack_mut().current_mut().jump_next_matching(config);
                self.prepare_preview(ctx);
                ctx.render()?;
            }
            CommonAction::PreviousResult => {
                self.stack_mut().current_mut().jump_previous_matching(config);
                self.prepare_preview(ctx);
                ctx.render()?;
            }
            CommonAction::InvertSelection => {
                self.stack_mut().current_mut().invert_marked();

                ctx.render()?;
            }
            CommonAction::Select => {
                self.stack_mut().current_mut().toggle_mark_selected();
                self.stack_mut()
                    .current_mut()
                    .next(ctx.config.scrolloff, ctx.config.wrap_navigation);
                self.prepare_preview(ctx);
                ctx.render()?;
            }
            CommonAction::Close if !self.stack().current().marked().is_empty() => {
                self.stack_mut().current_mut().marked_mut().clear();
                ctx.render()?;
            }
            CommonAction::Delete if !self.stack().current().marked().is_empty() => {
                for idx in self.stack().current().marked().iter().rev() {
                    let item = &self.stack().current().items[*idx];
                    self.delete(item, *idx, ctx)?;
                }

                ctx.render()?;
            }
            CommonAction::Delete => {
                if let Some((index, item)) = self.stack().current().selected_with_idx() {
                    self.delete(item, index, ctx)?;
                    ctx.render()?;
                }
            }
            CommonAction::Rename => {
                if let Some(item) = self.stack().current().selected() {
                    self.rename(item, ctx);
                }
            }
            CommonAction::FocusInput => {}
            CommonAction::Close => {}
            CommonAction::Confirm if self.stack().current().marked().is_empty() => {
                self.open(ctx)?;
                ctx.render()?;
            }
            CommonAction::ShowInfo => {
                if let Some(item) = self.stack().current().selected() {
                    self.show_info(item, ctx);
                }
            }
            CommonAction::Confirm => {}
            CommonAction::PaneDown => {}
            CommonAction::PaneUp => {}
            CommonAction::PaneRight => {}
            CommonAction::PaneLeft => {}
            CommonAction::AddOptions { kind: AddKind::Action(options) } => {
                let (enqueue, hovered_idx) = self.enqueue_items(options.all);
                if !enqueue.is_empty() {
                    let queue_len = ctx.queue.len();
                    let current_song_idx = ctx.find_current_song_in_queue().map(|(i, _)| i);

                    ctx.command(move |client| {
                        let autoplay = options.autoplay(queue_len, current_song_idx, hovered_idx);
                        client.enqueue_multiple(enqueue, options.position, autoplay)?;

                        Ok(())
                    });
                }
            }
            CommonAction::AddOptions { kind: AddKind::Modal(items) } => {
                let opts = items
                    .iter()
                    .map(|(label, opts)| {
                        let enqueue = self.enqueue_items(opts.all);
                        (label.to_owned(), *opts, enqueue)
                    })
                    .collect_vec();

                modal!(ctx, create_add_modal(opts, ctx));
            }
        }

        Ok(())
    }

    /// If `all` is true, returns `Enqueue` for all items in the current stack
    /// dir. Otherwise returns `Enqueue` for the currently hovered item if no
    /// items are marked or a list of `Enqueue` for all marked items.
    fn enqueue_items(&self, all: bool) -> (Vec<Enqueue>, Option<usize>) {
        if all {
            self.enqueue(self.stack().current().items.iter())
        } else if self.stack().current().marked().is_empty() {
            self.stack()
                .current()
                .selected()
                .map_or((Vec::new(), None), |item| self.enqueue(std::iter::once(item)))
        } else {
            self.enqueue(
                self.stack()
                    .current()
                    .marked()
                    .iter()
                    .map(|idx| &self.stack().current().items[*idx]),
            )
        }
    }
}
