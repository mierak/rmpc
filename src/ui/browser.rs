use anyhow::Result;
use crossterm::event::KeyCode;
use enum_map::EnumMap;
use itertools::Itertools;
use ratatui::{prelude::Rect, widgets::ListState};

use crate::{
    MpdQueryResult,
    config::keys::{
        CommonAction,
        GlobalAction,
        actions::{AddKind, Position, RateKind},
    },
    ctx::{Ctx, LIKE_STICKER, RATING_STICKER},
    mpd::{client::Client, commands::Song, mpd_client::MpdClient},
    shared::{
        key_event::KeyEvent,
        macros::modal,
        mouse_event::{MouseEvent, MouseEventKind, calculate_scrollbar_position},
        mpd_client_ext::{Autoplay, Enqueue, MpdClientExt, MpdDelete},
        mpd_query::EXTERNAL_COMMAND,
    },
    ui::{
        dirstack::{DirStack, DirStackItem, WalkDirStackItem},
        modals::{
            input_modal::InputModal,
            menu::{create_add_modal, create_rating_modal, modal::MenuModal},
            select_modal::SelectModal,
        },
        panes::Pane,
        widgets::browser::BrowserArea,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum MoveDirection {
    Up,
    Down,
}

#[allow(unused)]
pub(in crate::ui) trait BrowserPane<T>: Pane
where
    T: DirStackItem + std::fmt::Debug + Clone + Send + Sync + 'static,
{
    fn stack(&self) -> &DirStack<T, ListState>;
    fn stack_mut(&mut self) -> &mut DirStack<T, ListState>;
    fn browser_areas(&self) -> EnumMap<BrowserArea, Rect>;
    fn scrollbar_area(&self) -> Option<Rect> {
        let areas = self.browser_areas();
        let scrollbar = areas[BrowserArea::Scrollbar];
        if scrollbar.width > 0 { Some(scrollbar) } else { None }
    }
    fn set_filter_input_mode_active(&mut self, active: bool);
    fn is_filter_input_mode_active(&self) -> bool;
    fn next(&mut self, ctx: &Ctx) -> Result<()>;
    fn list_songs_in_item(
        &self,
        item: T,
    ) -> impl FnOnce(&mut Client<'_>) -> Result<Vec<Song>> + Send + Sync + Clone + 'static;
    fn fetch_data(&self, selected: &T, ctx: &Ctx) -> Result<()>;
    fn fetch_data_internal(&mut self, ctx: &Ctx) -> Result<()> {
        // Only attempt to fetch for empty directories
        if self.stack().next_dir_items().is_none_or(|d| d.is_empty())
            && let Some(selected) = self.stack().current().selected()
            && !selected.is_file()
        {
            self.fetch_data(selected, ctx)
        } else {
            Ok(())
        }
    }
    fn enqueue<'a>(&self, items: impl Iterator<Item = &'a T>) -> (Vec<Enqueue>, Option<usize>) {
        let path = self.stack().path();
        let hovered = self.stack().current().selected();
        let (items, idx) = items
            .flat_map(|item| item.walk(self.stack(), path.clone()))
            .enumerate()
            .fold((Vec::new(), None), |mut acc, (idx, item)| {
                let filename = item.as_path().to_owned();
                if let Some(hovered) = hovered
                    && hovered.is_file()
                    && hovered.as_path() == filename
                {
                    acc.1 = Some(idx);
                }
                acc.0.push(Enqueue::File { path: filename });

                acc
            });

        (items, idx)
    }
    fn open(&mut self, ctx: &Ctx) -> Result<()>;
    fn show_info(&self, item: &T, ctx: &Ctx) -> Result<()> {
        Ok(())
    }
    fn initial_playlist_name(&self) -> Option<String> {
        None
    }

    fn delete<'a>(&self, item: impl Iterator<Item = (usize, &'a T)>) -> Vec<MpdDelete> {
        Vec::new()
    }

    fn can_rename(&self, item: &T) -> bool {
        false
    }
    fn rename(item: &T, ctx: &Ctx) -> Result<()> {
        Ok(())
    }
    fn move_selected(&mut self, direction: MoveDirection, ctx: &Ctx) -> Result<()> {
        Ok(())
    }
    fn handle_filter_input(&mut self, event: &mut KeyEvent, ctx: &Ctx) -> Result<()> {
        if !self.is_filter_input_mode_active() {
            return Ok(());
        }

        let song_format = ctx.config.theme.browser_song_format.0.as_slice();
        let config = &ctx.config;
        match event.as_common_action(ctx) {
            Some(CommonAction::Close) => {
                self.set_filter_input_mode_active(false);
                self.stack_mut().current_mut().set_filter(None, song_format, ctx);
                self.fetch_data_internal(ctx);
            }
            Some(CommonAction::Confirm) => {
                self.set_filter_input_mode_active(false);
                ctx.render()?;
            }
            _ => {
                event.stop_propagation();
                match event.code() {
                    KeyCode::Char(c) => {
                        self.stack_mut().current_mut().push_filter(c, song_format, ctx);
                        self.stack_mut().current_mut().jump_first_matching(song_format, ctx);
                        self.fetch_data_internal(ctx);
                    }
                    KeyCode::Backspace => {
                        self.stack_mut().current_mut().pop_filter(song_format, ctx);
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

    /// checks if a mouse click is on the scrollbar area and also handles
    /// scrollbar interactions
    fn handle_scrollbar_interaction(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<bool> {
        let areas = self.browser_areas();
        let Some(scrollbar_area) = self.scrollbar_area() else {
            return Ok(false);
        };

        if !matches!(event.kind, MouseEventKind::LeftClick | MouseEventKind::Drag { .. }) {
            return Ok(false);
        }

        if let Some(perc) = calculate_scrollbar_position(event, scrollbar_area) {
            let current = self.stack_mut().current_mut().selected_with_idx().map(|(i, _)| i);
            self.stack_mut().current_mut().scroll_to(perc, ctx.config.scrolloff);
            if current != self.stack().current().selected_with_idx().map(|(i, _)| i) {
                self.fetch_data_internal(ctx);
            }
            ctx.render()?;
            return Ok(true);
        }

        Ok(false)
    }

    fn handle_mouse_action(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        if self.handle_scrollbar_interaction(event, ctx)? {
            return Ok(());
        }

        let areas = self.browser_areas();
        let prev_area = areas[BrowserArea::Previous];
        let current_area = areas[BrowserArea::Current];
        let preview_area = areas[BrowserArea::Preview];

        let position = event.into();
        let drag_start_position = match event.kind {
            MouseEventKind::Drag { drag_start_position } => Some(drag_start_position),
            _ => None,
        };
        match event.kind {
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick
                if prev_area.contains(position) =>
            {
                let clicked_row: usize = event.y.saturating_sub(prev_area.y).into();
                if let Some(prev_stack) = self.stack_mut().previous_mut() {
                    if let Some(idx_to_select) = prev_stack.state.get_at_rendered_row(clicked_row) {
                        prev_stack.select_idx(idx_to_select, ctx.config.scrolloff);
                    }
                    self.stack_mut().leave();
                    self.fetch_data_internal(ctx);
                }
            }
            MouseEventKind::DoubleClick if current_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) =
                    self.stack().current().state.get_at_rendered_row(clicked_row)
                {
                    self.next(ctx)?;
                    self.fetch_data_internal(ctx);
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

                    self.fetch_data_internal(ctx);
                }
            }
            MouseEventKind::LeftClick if current_area.contains(position) => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) =
                    self.stack().current().state.get_at_rendered_row(clicked_row)
                {
                    self.stack_mut().current_mut().select_idx(idx_to_select, ctx.config.scrolloff);
                    self.fetch_data_internal(ctx);
                }
            }
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick
                if preview_area.contains(position) =>
            {
                let clicked_row: usize = event.y.saturating_sub(preview_area.y).into();
                // Offset does not need to be accounted for since it is always
                // scrolled all the way to the top when going
                // deeper
                let idx_to_select = self.stack().next_dir_items().and_then(|preview| {
                    if clicked_row < preview.len() { Some(clicked_row) } else { None }
                });

                self.next(ctx)?;
                self.stack_mut().current_mut().select_idx(idx_to_select.unwrap_or_default(), 0);

                self.fetch_data_internal(ctx);
            }
            MouseEventKind::ScrollUp if current_area.contains(position) => {
                self.stack_mut().current_mut().scroll_up(1, ctx.config.scrolloff);
                self.fetch_data_internal(ctx);
            }
            MouseEventKind::ScrollDown if current_area.contains(position) => {
                self.stack_mut().current_mut().scroll_down(1, ctx.config.scrolloff);
                self.fetch_data_internal(ctx);
            }
            MouseEventKind::RightClick => {
                let clicked_row: usize = event.y.saturating_sub(current_area.y).into();

                if let Some(idx_to_select) =
                    self.stack().current().state.get_at_rendered_row(clicked_row)
                {
                    self.stack_mut().current_mut().select_idx(idx_to_select, ctx.config.scrolloff);
                    self.fetch_data_internal(ctx);
                }

                self.open_context_menu(ctx)?;
            }
            MouseEventKind::Drag { .. } => {}
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
                self.fetch_data_internal(ctx);
                ctx.render()?;
            }
            CommonAction::Down => {
                self.stack_mut().current_mut().next(config.scrolloff, config.wrap_navigation);
                self.fetch_data_internal(ctx);
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
                self.fetch_data_internal(ctx);
                ctx.render()?;
            }
            CommonAction::UpHalf => {
                self.stack_mut().current_mut().prev_half_viewport(ctx.config.scrolloff);
                self.fetch_data_internal(ctx);
                ctx.render()?;
            }
            CommonAction::PageUp => {
                self.stack_mut().current_mut().prev_viewport(ctx.config.scrolloff);
                self.fetch_data_internal(ctx);
                ctx.render()?;
            }
            CommonAction::PageDown => {
                self.stack_mut().current_mut().next_viewport(ctx.config.scrolloff);
                self.fetch_data_internal(ctx);
                ctx.render()?;
            }
            CommonAction::Bottom => {
                self.stack_mut().current_mut().last();
                self.fetch_data_internal(ctx);
                ctx.render()?;
            }
            CommonAction::Top => {
                self.stack_mut().current_mut().first();
                self.fetch_data_internal(ctx);
                ctx.render()?;
            }
            CommonAction::Right => {
                self.next(ctx)?;
                self.fetch_data_internal(ctx);
                ctx.render()?;
            }
            CommonAction::Left => {
                self.stack_mut().leave();
                self.fetch_data_internal(ctx);
                ctx.render()?;
            }
            CommonAction::EnterSearch => {
                self.set_filter_input_mode_active(true);
                self.stack_mut().current_mut().set_filter(
                    Some(String::new()),
                    ctx.config.theme.browser_song_format.0.as_slice(),
                    ctx,
                );

                ctx.render()?;
            }
            CommonAction::NextResult => {
                self.stack_mut()
                    .current_mut()
                    .jump_next_matching(ctx.config.theme.browser_song_format.0.as_slice(), ctx);
                self.fetch_data_internal(ctx);
                ctx.render()?;
            }
            CommonAction::PreviousResult => {
                self.stack_mut()
                    .current_mut()
                    .jump_previous_matching(ctx.config.theme.browser_song_format.0.as_slice(), ctx);
                self.fetch_data_internal(ctx);
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
                self.fetch_data_internal(ctx);
                ctx.render()?;
            }
            CommonAction::Close if !self.stack().current().marked().is_empty() => {
                self.stack_mut().current_mut().marked_mut().clear();
                ctx.render()?;
            }
            CommonAction::Delete => {
                let items = self.delete_items(false);
                if !items.is_empty() {
                    ctx.command(move |client| {
                        client.delete_multiple(items)?;
                        Ok(())
                    });
                    self.stack_mut().current_mut().marked_mut().clear();
                }
            }
            CommonAction::Rename => {
                if let Some(item) = self.stack().current().selected() {
                    Self::rename(item, ctx);
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
            CommonAction::ContextMenu => {
                self.open_context_menu(ctx)?;
            }
            CommonAction::Rate {
                kind: RateKind::Value(value),
                current: false,
                min_rating: _,
                max_rating: _,
            } => {
                let items = self.enqueue(self.items(false).map(|(_, i)| i)).0;
                ctx.command(move |client| {
                    client.set_sticker_multiple(RATING_STICKER, value.to_string(), items)?;
                    Ok(())
                });
            }
            CommonAction::Rate {
                kind: RateKind::Modal { values, custom, like },
                current: false,
                min_rating,
                max_rating,
            } => {
                let items = self.enqueue(self.items(false).map(|(_, i)| i)).0;
                modal!(
                    ctx,
                    create_rating_modal(
                        items,
                        values.as_slice(),
                        min_rating,
                        max_rating,
                        custom,
                        like,
                        ctx
                    )
                );
            }
            CommonAction::Rate { kind: RateKind::Like(), current: false, .. } => {
                let items = self.enqueue(self.items(false).map(|(_, i)| i)).0;
                ctx.command(move |client| {
                    client.set_sticker_multiple(LIKE_STICKER, "2".to_string(), items)?;
                    Ok(())
                });
            }
            CommonAction::Rate { kind: RateKind::Neutral(), current: false, .. } => {
                let items = self.enqueue(self.items(false).map(|(_, i)| i)).0;
                ctx.command(move |client| {
                    client.set_sticker_multiple(LIKE_STICKER, "1".to_string(), items)?;
                    Ok(())
                });
            }
            CommonAction::Rate { kind: RateKind::Dislike(), current: false, .. } => {
                let items = self.enqueue(self.items(false).map(|(_, i)| i)).0;
                ctx.command(move |client| {
                    client.set_sticker_multiple(LIKE_STICKER, "0".to_string(), items)?;
                    Ok(())
                });
            }
            CommonAction::Rate { kind: _, current: true, min_rating: _, max_rating: _ } => {
                event.abandon();
            }
        }

        Ok(())
    }

    fn items<'a>(&'a self, all: bool) -> Box<dyn Iterator<Item = (usize, &'a T)> + 'a> {
        if all {
            Box::new(self.stack().current().items.iter().enumerate())
        } else if self.stack().current().marked().is_empty() {
            if let Some((idx, item)) = self.stack().current().selected_with_idx() {
                Box::new(std::iter::once((idx, item)))
            } else {
                Box::new(std::iter::empty::<(usize, &T)>())
            }
        } else {
            Box::new(
                self.stack()
                    .current()
                    .marked()
                    .iter()
                    .map(|idx| (*idx, &self.stack().current().items[*idx])),
            )
        }
    }

    fn delete_items(&self, all: bool) -> Vec<MpdDelete> {
        self.delete(self.items(all))
    }

    /// If `all` is true, returns `Enqueue` for all items in the current stack
    /// dir. Otherwise returns `Enqueue` for the currently hovered item if no
    /// items are marked or a list of `Enqueue` for all marked items.
    fn enqueue_items(&self, all: bool) -> (Vec<Enqueue>, Option<usize>) {
        self.enqueue(self.items(all).map(|(_, item)| item))
    }

    fn open_context_menu(&mut self, ctx: &Ctx) -> Result<()> {
        let list_songs_in_items = self
            .items(false)
            .map(|(_, item)| self.list_songs_in_item(item.to_owned()))
            .collect_vec();

        let modal = MenuModal::new(ctx)
            .list_section(ctx, |mut section| {
                let (current_items, _) = self.enqueue_items(false);
                if !current_items.is_empty() {
                    let cloned_items = current_items.clone();
                    section.add_item("Add to queue", move |ctx| {
                        ctx.command(move |client| {
                            client.enqueue_multiple(
                                cloned_items,
                                Position::EndOfQueue,
                                Autoplay::None,
                            )?;
                            Ok(())
                        });
                        Ok(())
                    });
                    let cloned_items = current_items.clone();
                    section.add_item("Replace queue", move |ctx| {
                        ctx.command(move |client| {
                            client.enqueue_multiple(
                                cloned_items,
                                Position::Replace,
                                Autoplay::None,
                            )?;
                            Ok(())
                        });
                        Ok(())
                    });
                }

                let songs_in_items_clone = list_songs_in_items.clone();
                let initial_playlist_name = self.initial_playlist_name();
                section.add_item("Create playlist", move |ctx| {
                    modal!(
                        ctx,
                        InputModal::new(ctx)
                            .title("Create new playlist")
                            .confirm_label("Save")
                            .input_label("Playlist name:")
                            .initial_value(initial_playlist_name.unwrap_or_default())
                            .on_confirm(move |ctx, value| {
                                let value = value.to_owned();
                                ctx.command(move |client| {
                                    let items: Vec<_> = songs_in_items_clone
                                        .into_iter()
                                        .map(|cb| -> Result<_> { cb(client) })
                                        .collect::<Result<Vec<Vec<_>>>>()?
                                        .into_iter()
                                        .flatten()
                                        .collect();
                                    client.create_playlist(
                                        &value,
                                        items.into_iter().map(|s| s.file).collect(),
                                    )?;

                                    Ok(())
                                });
                                Ok(())
                            })
                    );
                    Ok(())
                });

                section.add_item("Add to playlist", move |ctx| {
                    let (items, playlists) = ctx.query_sync(move |client| {
                        let items: Vec<_> = list_songs_in_items
                            .into_iter()
                            .map(|cb| -> Result<_> { cb(client) })
                            .collect::<Result<Vec<Vec<_>>>>()?
                            .into_iter()
                            .flatten()
                            .collect();
                        let playlists = client.list_playlists()?;
                        Ok((items, playlists.into_iter().map(|p| p.name).collect_vec()))
                    })?;

                    modal!(
                        ctx,
                        SelectModal::builder()
                            .ctx(ctx)
                            .options(playlists)
                            .confirm_label("Add")
                            .title("Select a playlist")
                            .on_confirm(move |ctx, selected, _idx| {
                                ctx.command(move |client| {
                                    client.add_to_playlist_multiple(
                                        &selected,
                                        items.into_iter().map(|s| s.file).collect_vec(),
                                    )?;
                                    Ok(())
                                });
                                Ok(())
                            })
                            .build()
                    );
                    Ok(())
                });

                Some(section)
            })
            .list_section(ctx, |mut section| {
                let current_item = self.stack().current().selected().cloned();
                if let Some(item) = current_item {
                    let is_renameable =
                        self.stack().current().selected().is_some_and(|item| self.can_rename(item));
                    if is_renameable {
                        section.add_item("Rename", move |ctx| {
                            Self::rename(&item, ctx)?;
                            Ok(())
                        });
                    }
                }

                if section.items.is_empty() { None } else { Some(section) }
            })
            .list_section(ctx, |mut section| {
                // TODO Deletion cannot be currently done as we need to clear the marked items
                // after the deletion occurs but do not have access to the pane's state in the
                // callback. An event should be dispatched upon deletion to clear the items or
                // better yet, the marked items need to be refactored directly into the
                // `DirStackItem` directly.

                // if !to_delete.is_empty() {
                //     section.add_item("Delete", move |ctx| {
                //         if !to_delete.is_empty() {
                //             ctx.command(move |client| {
                //                 client.delete_multiple(to_delete)?;
                //                 Ok(())
                //             });
                //         }
                //         Ok(())
                //     });
                // }
                //
                // if !all_to_delete.is_empty() {
                //     section.add_item("Delete all", move |ctx| {
                //         if !all_to_delete.is_empty() {
                //             ctx.command(move |client| {
                //                 client.delete_multiple(all_to_delete)?;
                //                 Ok(())
                //             });
                //         }
                //         Ok(())
                //     });
                // }

                if section.items.is_empty() { None } else { Some(section) }
            })
            .list_section(ctx, |section| {
                let section = section.item("Cancel", |_ctx| Ok(()));
                Some(section)
            })
            .build();

        modal!(ctx, modal);
        Ok(())
    }
}

#[cfg(test)]
mod scrollbar_tests {
    use ratatui::layout::Rect;

    use crate::shared::mouse_event::{MouseEvent, MouseEventKind, calculate_scrollbar_position};

    #[test]
    fn test_calculate_scrollbar_index_with_new_function() {
        let scrollbar_area = Rect::new(29, 1, 1, 8);

        // Click at the top of the scrollbar (should go to first item)
        let event = MouseEvent { kind: MouseEventKind::LeftClick, x: 29, y: 1 };
        assert_eq!(calculate_scrollbar_position(event, scrollbar_area), Some(0.0));

        // Click at the bottom of the scrollbar (should go to last item)
        let event = MouseEvent { kind: MouseEventKind::LeftClick, x: 29, y: 8 };
        assert_eq!(calculate_scrollbar_position(event, scrollbar_area), Some(1.0));

        // Click in the middle
        let event = MouseEvent { kind: MouseEventKind::LeftClick, x: 29, y: 4 };
        assert_eq!(calculate_scrollbar_position(event, scrollbar_area), Some(3.0 / 7.0));
    }

    #[test]
    fn test_mouse_event_in_scrollbar_area() {
        let scrollbar_area = Rect::new(29, 1, 1, 8);

        let inside_event = MouseEvent { kind: MouseEventKind::LeftClick, x: 29, y: 3 };
        assert!(scrollbar_area.contains(inside_event.into()));

        let outside_event = MouseEvent { kind: MouseEventKind::LeftClick, x: 28, y: 3 };
        assert!(!scrollbar_area.contains(outside_event.into()));
    }

    #[test]
    fn test_scrollbar_drag_events() {
        let scrollbar_area = Rect::new(29, 1, 1, 8);
        let drag_start = ratatui::layout::Position { x: 29, y: 1 };
        let drag_event = MouseEvent {
            kind: MouseEventKind::Drag { drag_start_position: drag_start },
            x: 29,
            y: 5,
        };
        assert!(scrollbar_area.contains(drag_event.into()));
        assert!(matches!(drag_event.kind, MouseEventKind::Drag { .. }));
    }

    #[test]
    fn test_scrollbar_bottom_click_exact() {
        let scrollbar_area = Rect::new(29, 1, 1, 10);

        // Click at the bottom of the scrollbar
        let event = MouseEvent { kind: MouseEventKind::LeftClick, x: 29, y: 10 };
        assert_eq!(calculate_scrollbar_position(event, scrollbar_area), Some(1.0));

        // Click beyond the bottom of the scrollbar
        let event = MouseEvent { kind: MouseEventKind::LeftClick, x: 29, y: 15 };
        assert_eq!(calculate_scrollbar_position(event, scrollbar_area), None);
    }

    #[test]
    fn test_scrollbar_position_ratio_calculation() {
        let scrollbar_area = Rect::new(29, 1, 1, 10);

        let test_cases: Vec<(_, f64)> = vec![
            (1, 0.0),       // Top
            (3, 2.0 / 9.0), // ~20% position
            (5, 4.0 / 9.0), // ~40% position
            (6, 5.0 / 9.0), // ~50% position
            (8, 7.0 / 9.0), // ~70% position
            (10, 1.0),      // Bottom
        ];

        for (click_y, expected_target) in test_cases {
            let event = MouseEvent { kind: MouseEventKind::LeftClick, x: 29, y: click_y };
            let result = calculate_scrollbar_position(event, scrollbar_area);
            assert!(result.is_some());
            assert!(
                (result.expect("scrollbar calculation should return a value") - expected_target)
                    .abs()
                    < 0.0001
            );
        }
    }
}
