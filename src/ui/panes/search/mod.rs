use std::collections::HashSet;

use anyhow::Result;
use enum_map::EnumMap;
use itertools::Itertools;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    text::Span,
    widgets::{Block, Borders, List, ListItem, ListState, Padding},
};

use super::Pane;
use crate::{
    MpdQueryResult,
    config::{
        keys::{
            CommonAction,
            GlobalAction,
            actions::{AddKind, AutoplayKind, DeleteKind, Position, RateKind, SaveKind},
        },
        tabs::PaneType,
    },
    core::command::{create_env, run_external},
    ctx::{Ctx, LIKE_STICKER, RATING_STICKER},
    mpd::{
        client::Client,
        commands::Song,
        mpd_client::{Filter, MpdClient, MpdCommand},
        proto_client::ProtoClient,
        version::Version,
    },
    shared::{
        keys::ActionEvent,
        macros::{modal, status_error, status_info, status_warn},
        mouse_event::{MouseEvent, MouseEventKind, calculate_scrollbar_position},
        mpd_client_ext::{Enqueue, MpdClientExt},
    },
    ui::{
        UiEvent,
        dirstack::Dir,
        input::InputResultEvent,
        modals::{
            input_modal::InputModal,
            menu::{
                add_to_playlist_or_show_modal,
                create_add_modal,
                create_delete_modal,
                create_rating_modal,
                create_save_modal,
                delete_from_playlist_or_show_confirmation,
                modal::MenuModal,
            },
            select_modal::SelectModal,
        },
        panes::search::inputs::{ActionResult, InputGroups, InputType, TextboxInput},
        widgets::browser::BrowserArea,
    },
};

mod inputs;

#[derive(Debug)]
pub struct SearchPane {
    inputs: InputGroups,
    phase: Phase,
    songs_dir: Dir<Song, ListState>,
    column_areas: EnumMap<BrowserArea, Rect>,
}

const SEARCH: &str = "search";

impl SearchPane {
    pub fn new(ctx: &Ctx) -> Self {
        let config = &ctx.config;

        let inputs = InputGroups::builder()
            .ctx(ctx)
            .search_config(&config.search)
            .initial_fold_case(!config.search.case_sensitive)
            .initial_strip_diacritics(config.search.ignore_diacritics)
            .search_button(config.search.search_button)
            .text_style(config.as_text_style())
            .separator_style(config.theme.borders_style)
            .current_item_style(config.theme.current_item_style)
            .highlight_item_style(config.theme.highlighted_item_style)
            .stickers_supported(ctx.stickers_supported.into())
            .strip_diacritics_supported(ctx.mpd_version >= Version::new(0, 25, 0))
            .build();

        Self {
            phase: Phase::Search,
            songs_dir: Dir::default(),
            inputs,
            column_areas: EnumMap::default(),
        }
    }

    fn items<'a>(&'a self, all: bool) -> Box<dyn Iterator<Item = (usize, &'a Song)> + 'a> {
        if all {
            Box::new(self.songs_dir.items.iter().enumerate())
        } else if !self.songs_dir.marked().is_empty() {
            Box::new(self.songs_dir.marked().iter().map(|idx| (*idx, &self.songs_dir.items[*idx])))
        } else if let Some(item) = self.songs_dir.selected_with_idx() {
            Box::new(std::iter::once(item))
        } else {
            Box::new(std::iter::empty())
        }
    }

    fn enqueue(&self, all: bool) -> (Option<usize>, Vec<Enqueue>) {
        let items = self
            .items(all)
            .map(|(_, item)| Enqueue::File { path: item.file.clone() })
            .collect_vec();

        let hovered = self.songs_dir.selected().map(|s| s.file.as_str());
        let hovered_idx = if let Some(hovered) = hovered {
            items
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| {
                    if let Enqueue::File { path } = item { Some((idx, path)) } else { None }
                })
                .find(|(_, path)| path == &hovered)
                .map(|(idx, _)| idx)
        } else {
            None
        };

        (hovered_idx, items)
    }

    fn render_song_column(
        &mut self,
        frame: &mut ratatui::prelude::Frame<'_>,
        area: ratatui::prelude::Rect,
        ctx: &Ctx,
    ) {
        let config = &ctx.config;
        let column_right_padding: u16 = config.theme.scrollbar.is_some().into();
        let title = self.songs_dir.filter_text(area.width, ctx);

        let block = {
            let mut b = Block::default();
            if let Some(title) = title {
                b = b.title(title);
            }
            b.padding(Padding::new(0, column_right_padding, 0, 0))
        };
        let current = List::new(
            self.songs_dir.to_list_items(ctx.config.theme.browser_song_format.0.as_slice(), ctx),
        )
        .highlight_style(config.theme.current_item_style);
        let directory = &mut self.songs_dir;

        directory.state.set_content_and_viewport_len(directory.items.len(), area.height.into());
        if !directory.items.is_empty() && directory.state.get_selected().is_none() {
            directory.state.select(Some(0), 0);
        }
        let inner_block = block.inner(area);

        self.column_areas[BrowserArea::Current] = inner_block;
        self.column_areas[BrowserArea::Scrollbar] =
            if matches!(self.phase, Phase::BrowseResults) { area } else { Rect::default() };
        frame.render_widget(block, area);
        frame.render_stateful_widget(current, inner_block, directory.state.as_render_state_ref());
        if let Some(scrollbar) = config.as_styled_scrollbar() {
            frame.render_stateful_widget(
                scrollbar,
                self.column_areas[BrowserArea::Scrollbar],
                directory.state.as_scrollbar_state_ref(),
            );
        }
    }

    /// Trigger search if search should be done on any change. Does nothing when
    /// a dedicated search button is used.
    fn maybe_search_on_change(&mut self, ctx: &Ctx) {
        if !ctx.config.search.search_button {
            self.search(ctx);
        }
    }

    fn search(&mut self, ctx: &Ctx) {
        let search_mode = self.inputs.search_mode();
        let filter = self.inputs.inputs.iter().filter_map(|input| match &input {
            InputType::Textbox(TextboxInput { buffer_id, filter_key: Some(key), .. }) => {
                let value = ctx.input.value(*buffer_id).trim().to_owned();
                if !value.is_empty() && !key.is_empty() {
                    Some((key.to_owned(), value, search_mode))
                } else {
                    None
                }
            }
            _ => None,
        });

        let stickers_supported = ctx.stickers_supported.into();
        let fold_case = self.inputs.fold_case();
        let strip_diacritics = self.inputs.strip_diacritics();
        let liked_filter = self.inputs.liked_filter();

        let rating_filter = if self.inputs.is_rating_filter_active() {
            let Ok(rating_filter) = self.inputs.rating_filter(ctx) else {
                status_error!("Rating must be a valid integer {:?}", self.inputs.rating_value(ctx));
                return;
            };
            rating_filter
        } else {
            None
        };

        let mut filter = filter.collect_vec();

        if filter.is_empty()
            && stickers_supported
            && (rating_filter.is_some() || liked_filter.is_some())
        {
            // Filters are empty, but rating filters are set - show all songs with the
            // wanted rating
            ctx.query().id(SEARCH).replace_id(SEARCH).target(PaneType::Search).query(
                move |client| {
                    // empty URI returns all songs with the sticker
                    let uris = match (rating_filter, liked_filter) {
                        (Some(rf), Some(lf)) => {
                            let mut ratings: HashSet<_> = client
                                .find_stickers("", RATING_STICKER, Some(rf))?
                                .0
                                .into_iter()
                                .map(|s| s.file)
                                .collect();
                            let liked: HashSet<_> = client
                                .find_stickers("", LIKE_STICKER, Some(lf))?
                                .0
                                .into_iter()
                                .map(|s| s.file)
                                .collect();

                            // Do an intersection of both sets
                            ratings.retain(|uri| liked.contains(uri));

                            ratings
                        }
                        (Some(rf), None) => client
                            .find_stickers("", RATING_STICKER, Some(rf))?
                            .0
                            .into_iter()
                            .map(|s| s.file)
                            .collect(),
                        (None, Some(lf)) => client
                            .find_stickers("", LIKE_STICKER, Some(lf))?
                            .0
                            .into_iter()
                            .map(|s| s.file)
                            .collect(),
                        (None, None) => HashSet::new(),
                    };

                    client.send_start_cmd_list()?;
                    for uri in uris {
                        client.send_lsinfo(Some(&uri))?;
                    }
                    client.send_execute_cmd_list()?;
                    let data: Vec<Song> = client.read_response()?;

                    Ok(MpdQueryResult::SearchResult { data })
                },
            );
        } else if filter.is_empty() {
            // Filters are empty, stickers are either not supported or not set - clear
            // current results
            let _ = std::mem::take(&mut self.songs_dir);
        } else {
            // Search normally
            ctx.query().id(SEARCH).replace_id(SEARCH).target(PaneType::Search).query(
                move |client| {
                    let filter = filter
                        .iter_mut()
                        .map(|&mut (ref mut key, ref value, ref mut kind)| {
                            Filter::new(std::mem::take(key), value).with_type((*kind).into())
                        })
                        .collect_vec();

                    let data = if fold_case {
                        client.search(&filter, strip_diacritics)
                    } else {
                        client.find(&filter)
                    }?;

                    let data = if stickers_supported && rating_filter.is_some() {
                        // empty URI returns all songs with the sticker
                        let ratings = client.find_stickers("", RATING_STICKER, rating_filter)?;
                        let ratings: HashSet<_> = ratings.into_iter().map(|r| r.file).collect();
                        data.into_iter().filter(|song| ratings.contains(&song.file)).collect()
                    } else {
                        data
                    };

                    let data = if stickers_supported && liked_filter.is_some() {
                        // empty URI returns all songs with the sticker
                        let liked = client.find_stickers("", LIKE_STICKER, liked_filter)?;
                        let liked: HashSet<_> = liked.into_iter().map(|r| r.file).collect();
                        data.into_iter().filter(|song| liked.contains(&song.file)).collect()
                    } else {
                        data
                    };

                    Ok(MpdQueryResult::SearchResult { data })
                },
            );
        }
    }

    fn handle_search_phase_action(&mut self, event: &mut ActionEvent, ctx: &mut Ctx) -> Result<()> {
        let config = &ctx.config;
        if let Some(action) = event.claim_global() {
            if let GlobalAction::ExternalCommand { command, .. } = action {
                let songs = self.songs_dir.items.iter().map(|song| song.file.as_str());
                run_external(command.clone(), create_env(ctx, songs));
            } else {
                event.abandon();
            }
        } else if let Some(action) = event.claim_common() {
            match action.to_owned() {
                CommonAction::Down => {
                    if config.wrap_navigation {
                        self.inputs.next();
                    } else {
                        self.inputs.next_non_wrapping();
                    }

                    ctx.render()?;
                }
                CommonAction::Up => {
                    if config.wrap_navigation {
                        self.inputs.prev();
                    } else {
                        self.inputs.prev_non_wrapping();
                    }

                    ctx.render()?;
                }
                CommonAction::MoveDown => {}
                CommonAction::MoveUp => {}
                CommonAction::DownHalf => {}
                CommonAction::UpHalf => {}
                CommonAction::PageDown => {}
                CommonAction::PageUp => {}
                CommonAction::Right if !self.songs_dir.items.is_empty() => {
                    self.phase = Phase::BrowseResults;

                    ctx.render()?;
                }
                CommonAction::Right => {}
                CommonAction::Left => {}
                CommonAction::Top => {
                    self.inputs.first();

                    ctx.render()?;
                }
                CommonAction::Bottom => {
                    self.inputs.last();

                    ctx.render()?;
                }
                CommonAction::EnterSearch => {}
                CommonAction::NextResult => {}
                CommonAction::PreviousResult => {}
                CommonAction::Select => {}
                CommonAction::InvertSelection => {}
                CommonAction::Rename => {}
                CommonAction::Close => {}
                CommonAction::Confirm => {
                    match self.inputs.activate_focused(ctx) {
                        ActionResult::Search => {
                            self.search(ctx);
                        }
                        ActionResult::Reset => {
                            self.inputs.reset_focused(ctx);
                            self.songs_dir = Dir::default();
                        }
                        ActionResult::None => {}
                    }
                    ctx.render()?;
                }
                CommonAction::FocusInput => {
                    self.inputs.enter_insert_mode(ctx);
                    ctx.render()?;
                }
                // Modal while we are on search column does not support all options. It can
                // be implemented later.
                CommonAction::AddOptions { kind: AddKind::Modal(_) } => {}
                CommonAction::AddOptions { kind: AddKind::Action(opts) } if opts.all => {
                    let (_, enqueue) = self.enqueue(opts.all);
                    if !enqueue.is_empty() {
                        let current_song_idx = ctx.find_current_song_in_queue().map(|(i, _)| i);
                        Client::resolve_and_enqueue(
                            ctx,
                            enqueue,
                            opts.position,
                            opts.autoplay,
                            current_song_idx,
                            None,
                        );
                    }
                }
                // This action only makes sense when opts.all is true while we are on the
                // search column.
                CommonAction::AddOptions { kind: AddKind::Action(_) } => {}
                CommonAction::Delete => {
                    self.inputs.reset_focused(ctx);
                    self.songs_dir = Dir::default();
                    ctx.render()?;
                }
                CommonAction::PaneDown => {}
                CommonAction::PaneUp => {}
                CommonAction::PaneRight => {}
                CommonAction::PaneLeft => {}
                CommonAction::ShowInfo => {}
                CommonAction::ContextMenu => {}
                CommonAction::Rate { kind: _, min_rating: _, max_rating: _, current: true } => {
                    event.abandon();
                }
                CommonAction::Rate { .. } => {}
                CommonAction::Save {
                    kind: SaveKind::Playlist { name, all: true, duplicates_strategy },
                } => {
                    let song_paths: Vec<String> =
                        self.items(true).map(|(_, song)| song.file.clone()).collect();
                    if song_paths.is_empty() {
                        status_warn!("No songs selected to save");
                        return Ok(());
                    }

                    add_to_playlist_or_show_modal(name, song_paths, duplicates_strategy, ctx);
                }
                CommonAction::Save { kind: SaveKind::Modal { all: true, duplicates_strategy } } => {
                    let song_paths: Vec<String> =
                        self.items(true).map(|(_, song)| song.file.clone()).collect();
                    if song_paths.is_empty() {
                        status_warn!("No songs selected to save");
                        return Ok(());
                    }

                    let modal = create_save_modal(song_paths, None, duplicates_strategy, ctx)?;
                    modal!(ctx, modal);
                }
                CommonAction::Save { .. } => {}
                CommonAction::DeleteFromPlaylist {
                    kind: DeleteKind::Playlist { name, all: true, confirmation },
                } => {
                    let song_paths: HashSet<String> =
                        self.items(true).map(|(_, song)| song.file.clone()).collect();
                    if song_paths.is_empty() {
                        status_warn!("No songs selected to delete");
                        return Ok(());
                    }

                    delete_from_playlist_or_show_confirmation(
                        name,
                        &song_paths,
                        confirmation,
                        ctx,
                    )?;
                }
                CommonAction::DeleteFromPlaylist {
                    kind: DeleteKind::Modal { all: true, confirmation },
                } => {
                    let song_paths: HashSet<_> =
                        self.items(true).map(|(_, song)| song.file.clone()).collect();
                    if song_paths.is_empty() {
                        status_warn!("No songs selected to delete");
                        return Ok(());
                    }

                    let modal = create_delete_modal(song_paths, confirmation, ctx)?;
                    modal!(ctx, modal);
                }
                CommonAction::DeleteFromPlaylist { .. } => {}
            }
        }

        Ok(())
    }

    fn handle_result_phase_action(&mut self, event: &mut ActionEvent, ctx: &mut Ctx) -> Result<()> {
        let Phase::BrowseResults = &mut self.phase else {
            return Ok(());
        };
        if let Some(action) = event.claim_global() {
            match action {
                GlobalAction::ExternalCommand { command, .. }
                    if !self.songs_dir.marked().is_empty() =>
                {
                    let songs = self.songs_dir.marked_items().map(|song| song.file.as_str());
                    run_external(command.clone(), create_env(ctx, songs));
                }
                GlobalAction::ExternalCommand { command, .. } => {
                    let selected = self.songs_dir.selected().map(|s| s.file.as_str());
                    run_external(command.clone(), create_env(ctx, selected));
                }
                _ => {
                    event.abandon();
                }
            }
        } else if let Some(action) = event.claim_common() {
            match action.to_owned() {
                CommonAction::Down => {
                    self.songs_dir.next(ctx.config.scrolloff, ctx.config.wrap_navigation);

                    ctx.render()?;
                }
                CommonAction::Up => {
                    self.songs_dir.prev(ctx.config.scrolloff, ctx.config.wrap_navigation);

                    ctx.render()?;
                }
                CommonAction::MoveDown => {}
                CommonAction::MoveUp => {}
                CommonAction::DownHalf => {
                    self.songs_dir.next_half_viewport(ctx.config.scrolloff);

                    ctx.render()?;
                }
                CommonAction::UpHalf => {
                    self.songs_dir.prev_half_viewport(ctx.config.scrolloff);

                    ctx.render()?;
                }
                CommonAction::PageDown => {
                    self.songs_dir.next_viewport(ctx.config.scrolloff);

                    ctx.render()?;
                }
                CommonAction::PageUp => {
                    self.songs_dir.prev_viewport(ctx.config.scrolloff);

                    ctx.render()?;
                }
                CommonAction::Right => {
                    let items = self.songs_dir.selected().map_or_else(Vec::new, |item| {
                        vec![Enqueue::File { path: item.file.clone() }]
                    });
                    if !items.is_empty() {
                        ctx.command(move |client| {
                            client.enqueue_multiple(items, None, None, false)?;
                            Ok(())
                        });
                    }
                }
                CommonAction::Left => {
                    self.phase = Phase::Search;

                    ctx.render()?;
                }
                CommonAction::Top => {
                    self.songs_dir.first();

                    ctx.render()?;
                }
                CommonAction::Bottom => {
                    self.songs_dir.last();

                    ctx.render()?;
                }
                CommonAction::EnterSearch => {
                    self.songs_dir.set_filter_active(true);
                    ctx.input.insert_mode(self.songs_dir.filter_buffer_id);
                    ctx.input.clear_buffer(self.songs_dir.filter_buffer_id);

                    ctx.render()?;
                }
                CommonAction::NextResult => {
                    self.songs_dir
                        .jump_next_matching(ctx.config.theme.browser_song_format.0.as_slice(), ctx);

                    ctx.render()?;
                }
                CommonAction::PreviousResult => {
                    self.songs_dir.jump_previous_matching(
                        ctx.config.theme.browser_song_format.0.as_slice(),
                        ctx,
                    );

                    ctx.render()?;
                }
                CommonAction::Select => {
                    self.songs_dir.toggle_mark_selected();
                    self.songs_dir.next(ctx.config.scrolloff, ctx.config.wrap_navigation);

                    ctx.render()?;
                }
                CommonAction::InvertSelection => {
                    self.songs_dir.invert_marked();

                    ctx.render()?;
                }
                CommonAction::Close if !self.songs_dir.marked().is_empty() => {
                    self.songs_dir.marked_mut().clear();
                    ctx.render()?;
                }
                CommonAction::Rename => {}
                CommonAction::Close => {}
                CommonAction::Confirm if self.songs_dir.marked().is_empty() => {
                    let (hovered_song_idx, items) = self.enqueue(true);
                    let current_song_idx = ctx.find_current_song_in_queue().map(|(i, _)| i);

                    if !items.is_empty() {
                        Client::resolve_and_enqueue(
                            ctx,
                            items,
                            Position::Replace,
                            AutoplayKind::Hovered,
                            current_song_idx,
                            hovered_song_idx,
                        );
                    }

                    ctx.render()?;
                }
                CommonAction::Confirm => {}
                CommonAction::FocusInput => {}
                CommonAction::AddOptions { kind: AddKind::Action(opts) } => {
                    let (hovered_song_idx, enqueue) = self.enqueue(opts.all);

                    if !enqueue.is_empty() {
                        let current_song_idx = ctx.find_current_song_in_queue().map(|(i, _)| i);

                        Client::resolve_and_enqueue(
                            ctx,
                            enqueue,
                            opts.position,
                            opts.autoplay,
                            current_song_idx,
                            hovered_song_idx,
                        );
                    }
                }
                CommonAction::AddOptions { kind: AddKind::Modal(opts) } => {
                    let opts = opts
                        .iter()
                        .map(|(label, opts)| {
                            let (hovered_song_idx, enqueue) = self.enqueue(opts.all);

                            (label.to_owned(), *opts, (enqueue, hovered_song_idx))
                        })
                        .collect_vec();

                    modal!(ctx, create_add_modal(opts, ctx));
                }
                CommonAction::Delete => {}
                CommonAction::PaneDown => {}
                CommonAction::PaneUp => {}
                CommonAction::PaneRight => {}
                CommonAction::PaneLeft => {}
                CommonAction::ShowInfo => {}
                CommonAction::ContextMenu => {
                    self.open_result_phase_context_menu(ctx);
                }
                CommonAction::Rate {
                    kind: RateKind::Value(value),
                    current: false,
                    min_rating: _,
                    max_rating: _,
                } => {
                    let items = self.enqueue(false).1;
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
                    let items = self.enqueue(false).1;
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
                    let items = self.enqueue(false).1;
                    ctx.command(move |client| {
                        client.set_sticker_multiple(LIKE_STICKER, "2".to_string(), items)?;
                        Ok(())
                    });
                }
                CommonAction::Rate { kind: RateKind::Neutral(), current: false, .. } => {
                    let items = self.enqueue(false).1;
                    ctx.command(move |client| {
                        client.set_sticker_multiple(LIKE_STICKER, "1".to_string(), items)?;
                        Ok(())
                    });
                }
                CommonAction::Rate { kind: RateKind::Dislike(), current: false, .. } => {
                    let items = self.enqueue(false).1;
                    ctx.command(move |client| {
                        client.set_sticker_multiple(LIKE_STICKER, "0".to_string(), items)?;
                        Ok(())
                    });
                }
                CommonAction::Rate { kind: _, current: true, min_rating: _, max_rating: _ } => {
                    event.abandon();
                }
                CommonAction::Save {
                    kind: SaveKind::Playlist { name, all, duplicates_strategy },
                } => {
                    let song_paths: Vec<String> =
                        self.items(all).map(|(_, song)| song.file.clone()).collect();
                    if song_paths.is_empty() {
                        status_warn!("No songs selected to save");
                        return Ok(());
                    }

                    add_to_playlist_or_show_modal(name, song_paths, duplicates_strategy, ctx);
                }
                CommonAction::Save { kind: SaveKind::Modal { all, duplicates_strategy } } => {
                    let song_paths: Vec<_> =
                        self.items(all).map(|(_, song)| song.file.clone()).collect();
                    if song_paths.is_empty() {
                        status_warn!("No songs selected to save");
                        return Ok(());
                    }

                    let modal = create_save_modal(song_paths, None, duplicates_strategy, ctx)?;
                    modal!(ctx, modal);
                }
                CommonAction::DeleteFromPlaylist {
                    kind: DeleteKind::Playlist { name, all, confirmation },
                } => {
                    let song_paths: HashSet<String> =
                        self.items(all).map(|(_, song)| song.file.clone()).collect();
                    if song_paths.is_empty() {
                        status_warn!("No songs selected to delete");
                        return Ok(());
                    }

                    delete_from_playlist_or_show_confirmation(
                        name,
                        &song_paths,
                        confirmation,
                        ctx,
                    )?;
                }
                CommonAction::DeleteFromPlaylist {
                    kind: DeleteKind::Modal { all, confirmation },
                } => {
                    let song_paths: HashSet<_> =
                        self.items(all).map(|(_, song)| song.file.clone()).collect();
                    if song_paths.is_empty() {
                        status_warn!("No songs selected to delete");
                        return Ok(());
                    }

                    let modal = create_delete_modal(song_paths, confirmation, ctx)?;
                    modal!(ctx, modal);
                }
            }
        }

        Ok(())
    }

    fn open_result_phase_context_menu(&self, ctx: &Ctx) {
        let modal = MenuModal::new(ctx)
            .list_section(ctx, move |mut section| {
                if !self.songs_dir.items.is_empty() {
                    let (_, enqueue) = self.enqueue(true);
                    if !enqueue.is_empty() {
                        let enqueue_clone = enqueue.clone();
                        section.add_item("Add all to queue", move |ctx| {
                            ctx.command(move |client| {
                                client.enqueue_multiple(enqueue_clone, None, None, false)?;
                                Ok(())
                            });
                            Ok(())
                        });
                        section.add_item("Replace queue with all", move |ctx| {
                            ctx.command(move |client| {
                                client.enqueue_multiple(enqueue, None, None, true)?;
                                Ok(())
                            });
                            Ok(())
                        });

                        let song_files =
                            self.items(true).map(|(_, item)| item.file.clone()).collect();
                        section.add_item("Create playlist from all", move |ctx| {
                            modal!(
                                ctx,
                                InputModal::new(ctx)
                                    .title("Create new playlist")
                                    .confirm_label("Save")
                                    .input_label("Playlist name:")
                                    .on_confirm(move |ctx, value| {
                                        let value = value.to_owned();
                                        ctx.command(move |client| {
                                            client.create_playlist(&value, song_files)?;
                                            Ok(())
                                        });
                                        Ok(())
                                    })
                            );
                            Ok(())
                        });

                        let song_files =
                            self.items(true).map(|(_, item)| item.file.clone()).collect();
                        section.add_item("Add all to playlist", move |ctx| {
                            let playlists = ctx.query_sync(move |client| {
                                Ok(client
                                    .list_playlists()?
                                    .into_iter()
                                    .map(|p| p.name)
                                    .collect_vec())
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
                                            client
                                                .add_to_playlist_multiple(&selected, song_files)?;
                                            Ok(())
                                        });
                                        Ok(())
                                    })
                                    .build()
                            );
                            Ok(())
                        });
                    }
                }
                Some(section)
            })
            .list_section(ctx, |mut section| {
                let song_files = self.items(false).map(|(_, item)| item.file.clone()).collect();
                section.add_item("Create playlist", move |ctx| {
                    modal!(
                        ctx,
                        InputModal::new(ctx)
                            .title("Create new playlist")
                            .confirm_label("Save")
                            .input_label("Playlist name:")
                            .on_confirm(move |ctx, value| {
                                let value = value.to_owned();
                                ctx.command(move |client| {
                                    client.create_playlist(&value, song_files)?;
                                    Ok(())
                                });
                                Ok(())
                            })
                    );
                    Ok(())
                });

                let song_files = self.items(false).map(|(_, item)| item.file.clone()).collect();
                section.add_item("Add to playlist", move |ctx| {
                    let playlists = ctx.query_sync(move |client| {
                        Ok(client.list_playlists()?.into_iter().map(|p| p.name).collect_vec())
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
                                    client.add_to_playlist_multiple(&selected, song_files)?;
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
                section.add_item("Cancel", |_| Ok(()));
                Some(section)
            })
            .build();
        modal!(ctx, modal);
    }

    fn scrollbar_area(&self) -> Option<Rect> {
        let area = self.column_areas[BrowserArea::Scrollbar];
        if area.width > 0 { Some(area) } else { None }
    }

    fn handle_scrollbar_interaction(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<bool> {
        if !matches!(self.phase, Phase::BrowseResults) {
            return Ok(false);
        }
        let Some(_) = ctx.config.theme.scrollbar else {
            return Ok(false);
        };
        let Some(scrollbar_area) = self.scrollbar_area() else {
            return Ok(false);
        };
        if !matches!(event.kind, MouseEventKind::LeftClick | MouseEventKind::Drag { .. }) {
            return Ok(false);
        }

        if let Some(perc) = calculate_scrollbar_position(event, scrollbar_area) {
            self.songs_dir.scroll_to(perc, ctx.config.scrolloff);
            ctx.render()?;
            return Ok(true);
        }

        Ok(false)
    }
}

impl Pane for SearchPane {
    fn render(
        &mut self,
        frame: &mut ratatui::prelude::Frame,
        area: ratatui::prelude::Rect,
        ctx: &Ctx,
    ) -> anyhow::Result<()> {
        let widths = &ctx.config.theme.column_widths;
        let [previous_area, current_area_init, preview_area] = *Layout::horizontal([
            Constraint::Percentage(widths[0]),
            Constraint::Percentage(widths[1]),
            Constraint::Percentage(widths[2]),
        ])
        .split(area) else {
            return Ok(());
        };

        frame.render_widget(
            Block::default().borders(Borders::RIGHT).border_style(ctx.config.theme.borders_style),
            previous_area,
        );
        frame.render_widget(
            Block::default().borders(Borders::RIGHT).border_style(ctx.config.theme.borders_style),
            current_area_init,
        );
        let previous_area = Rect {
            x: previous_area.x,
            y: previous_area.y,
            width: previous_area.width.saturating_sub(1),
            height: previous_area.height,
        };
        let current_area = Rect {
            x: current_area_init.x,
            y: current_area_init.y,
            width: current_area_init.width.saturating_sub(1),
            height: current_area_init.height,
        };

        match self.phase {
            Phase::Search => {
                self.column_areas[BrowserArea::Current] = current_area;
                self.inputs.render(current_area, frame.buffer_mut(), ctx);

                // Render only the part of the preview that is actually supposed to be shown
                let offset = self.songs_dir.state.offset();
                let items = self.songs_dir.to_list_items_range(
                    offset..offset + previous_area.height as usize,
                    ctx.config.theme.browser_song_format.0.as_slice(),
                    ctx,
                );
                let preview = List::new(items).style(ctx.config.as_text_style());
                frame.render_widget(preview, preview_area);
            }
            Phase::BrowseResults => {
                self.render_song_column(frame, current_area, ctx);
                self.inputs.render(previous_area, frame.buffer_mut(), ctx);
                if let Some(song) = self.songs_dir.selected() {
                    let preview = song.to_preview(
                        ctx.config.theme.preview_label_style,
                        ctx.config.theme.preview_metadata_group_style,
                        ctx,
                    );
                    let mut result = Vec::new();
                    for group in preview {
                        if let Some(name) = group.name {
                            result.push(ListItem::new(name).yellow().bold());
                        }
                        result.extend(group.items.clone());
                        result.push(ListItem::new(Span::raw("")));
                    }
                    let preview = List::new(result).style(ctx.config.as_text_style());
                    frame.render_widget(preview, preview_area);
                }
            }
        }

        self.column_areas[BrowserArea::Previous] = previous_area;
        self.column_areas[BrowserArea::Preview] = preview_area;

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, _is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::Database => {
                self.songs_dir = Dir::default();
                self.phase = Phase::Search;

                status_warn!(
                    "The music database has been updated. The current tab has been reinitialized in the root directory to prevent inconsistent behaviours."
                );
            }
            UiEvent::Reconnected => {
                self.phase = Phase::Search;
                self.songs_dir = Dir::default();
            }
            UiEvent::ConfigChanged => {
                *self = Self::new(ctx);
            }
            _ => {}
        }
        Ok(())
    }

    fn on_query_finished(
        &mut self,
        id: &'static str,
        data: MpdQueryResult,
        _is_visible: bool,
        ctx: &Ctx,
    ) -> Result<()> {
        match (id, data) {
            (SEARCH, MpdQueryResult::SearchResult { data }) => {
                status_info!("Found {} matching songs", data.len());
                self.songs_dir = Dir::new(data);
                ctx.render()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        if self.handle_scrollbar_interaction(event, ctx)? {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::LeftClick
                if self.column_areas[BrowserArea::Previous].contains(event.into()) =>
            {
                self.phase = Phase::Search;
                self.inputs.focus_input_at(event.into());
                ctx.render()?;
            }
            MouseEventKind::LeftClick
                if self.column_areas[BrowserArea::Preview].contains(event.into()) =>
            {
                match self.phase {
                    Phase::Search => {
                        if !self.songs_dir.items.is_empty() {
                            self.phase = Phase::BrowseResults;

                            let clicked_row: usize = event
                                .y
                                .saturating_sub(self.column_areas[BrowserArea::Preview].y)
                                .into();
                            if let Some(idx_to_select) =
                                self.songs_dir.state.get_at_rendered_row(clicked_row)
                            {
                                self.songs_dir.state.set_content_and_viewport_len(
                                    self.songs_dir.items.len(),
                                    self.column_areas[BrowserArea::Preview].height as usize,
                                );
                                self.songs_dir.select_idx(idx_to_select, ctx.config.scrolloff);
                            }

                            ctx.render()?;
                        }
                    }
                    Phase::BrowseResults => {
                        let (_, items) = self.enqueue(false);
                        if !items.is_empty() {
                            ctx.command(move |client| {
                                client.enqueue_multiple(items, None, None, false)?;
                                Ok(())
                            });
                        }
                    }
                }
            }
            MouseEventKind::LeftClick
                if self.column_areas[BrowserArea::Current].contains(event.into()) =>
            {
                match self.phase {
                    Phase::Search => {
                        if ctx.input.is_insert_mode() {
                            self.phase = Phase::Search;
                            ctx.input.normal_mode();
                            self.maybe_search_on_change(ctx);
                        }

                        self.inputs.focus_input_at(event.into());
                        ctx.render()?;
                    }
                    Phase::BrowseResults => {
                        let clicked_row = event
                            .y
                            .saturating_sub(self.column_areas[BrowserArea::Current].y)
                            .into();

                        if let Some(idx) = self.songs_dir.state.get_at_rendered_row(clicked_row) {
                            self.songs_dir.select_idx(idx, ctx.config.scrolloff);

                            ctx.render()?;
                        }
                    }
                }
            }
            MouseEventKind::DoubleClick => match self.phase {
                Phase::Search => {
                    if self.column_areas[BrowserArea::Current].contains(event.into()) {
                        match self.inputs.activate_focused(ctx) {
                            ActionResult::Search => {
                                self.search(ctx);
                            }
                            ActionResult::Reset => {
                                self.inputs.reset_focused(ctx);
                                self.songs_dir = Dir::default();
                            }
                            ActionResult::None => {}
                        }
                    }
                    ctx.render()?;
                }
                Phase::BrowseResults => {
                    let (_, items) = self.enqueue(false);
                    if !items.is_empty() {
                        ctx.command(move |client| {
                            client.enqueue_multiple(items, None, None, false)?;
                            Ok(())
                        });
                    }
                }
            },
            MouseEventKind::MiddleClick
                if self.column_areas[BrowserArea::Current].contains(event.into()) =>
            {
                match self.phase {
                    Phase::Search => {}
                    Phase::BrowseResults => {
                        let clicked_row = event
                            .y
                            .saturating_sub(self.column_areas[BrowserArea::Current].y)
                            .into();
                        if let Some(idx) = self.songs_dir.state.get_at_rendered_row(clicked_row) {
                            self.songs_dir.select_idx(idx, ctx.config.scrolloff);
                            self.songs_dir.select_idx(idx, ctx.config.scrolloff);
                            if let Some(item) = self.songs_dir.selected() {
                                let item = item.file.clone();
                                ctx.command(move |client| {
                                    client.add(&item, None)?;
                                    status_info!("Added '{item}' to queue");
                                    Ok(())
                                });
                            }
                            ctx.render()?;
                        }
                    }
                }
            }
            MouseEventKind::ScrollDown => match self.phase {
                Phase::Search => {
                    if ctx.input.is_insert_mode() {
                        ctx.input.normal_mode();
                        self.phase = Phase::Search;
                        self.maybe_search_on_change(ctx);
                    }
                    self.inputs.next_non_wrapping();
                    ctx.render()?;
                }
                Phase::BrowseResults => {
                    self.songs_dir.scroll_down(ctx.config.scroll_amount, ctx.config.scrolloff);
                    ctx.render()?;
                }
            },
            MouseEventKind::ScrollUp => match self.phase {
                Phase::Search => {
                    if ctx.input.is_insert_mode() {
                        ctx.input.normal_mode();
                        self.phase = Phase::Search;
                        self.maybe_search_on_change(ctx);
                    }
                    self.inputs.prev_non_wrapping();
                    ctx.render()?;
                }
                Phase::BrowseResults => {
                    self.songs_dir.scroll_up(ctx.config.scroll_amount, ctx.config.scrolloff);
                    ctx.render()?;
                }
            },
            MouseEventKind::RightClick => match self.phase {
                Phase::BrowseResults if !ctx.input.is_active(self.songs_dir.filter_buffer_id) => {
                    let clicked_row =
                        event.y.saturating_sub(self.column_areas[BrowserArea::Current].y).into();
                    if let Some(idx) = self.songs_dir.state.get_at_rendered_row(clicked_row) {
                        self.songs_dir.select_idx(idx, ctx.config.scrolloff);
                        ctx.render()?;
                    }
                    self.open_result_phase_context_menu(ctx);
                }
                _ => {}
            },
            MouseEventKind::Drag { .. } => {
                // drag events are handled by scrollbar interaction, no
                // additional action needed
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_insert_mode(&mut self, kind: InputResultEvent, ctx: &mut Ctx) -> Result<()> {
        match self.phase {
            Phase::Search => match kind {
                InputResultEvent::Push => {}
                InputResultEvent::Pop => {}
                InputResultEvent::Confirm => {
                    self.maybe_search_on_change(ctx);
                }
                InputResultEvent::NoChange => {}
                InputResultEvent::Cancel => {
                    self.maybe_search_on_change(ctx);
                }
            },
            Phase::BrowseResults => {
                let song_format = ctx.config.theme.browser_song_format.0.as_slice();
                match kind {
                    InputResultEvent::Push => {
                        self.songs_dir.recalculate_matched_items(song_format, ctx);
                        self.songs_dir.jump_first_matching(song_format, ctx);
                    }
                    InputResultEvent::Pop => {
                        self.songs_dir.recalculate_matched_items(song_format, ctx);
                    }
                    InputResultEvent::Confirm => {}
                    InputResultEvent::NoChange => {}
                    InputResultEvent::Cancel => {
                        self.songs_dir.set_filter_active(false);
                        ctx.input.clear_buffer(self.songs_dir.filter_buffer_id);
                    }
                }
            }
        }
        ctx.render()?;
        Ok(())
    }

    fn handle_action(&mut self, event: &mut ActionEvent, ctx: &mut Ctx) -> Result<()> {
        match &mut self.phase {
            Phase::Search => {
                self.handle_search_phase_action(event, ctx)?;
            }
            Phase::BrowseResults => {
                self.handle_result_phase_action(event, ctx)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
enum Phase {
    Search,
    BrowseResults,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_search_pane_scrollbar_calculation() {
        let scrollbar_height: u16 = 10;
        let total_items: usize = 50;

        let clicked_y = scrollbar_height.saturating_sub(1);
        let target_idx = if clicked_y >= scrollbar_height.saturating_sub(1) {
            total_items.saturating_sub(1)
        } else {
            let position_ratio =
                f64::from(clicked_y) / f64::from(scrollbar_height.saturating_sub(1));
            ((position_ratio * (total_items.saturating_sub(1)) as f64) as usize)
                .min(total_items.saturating_sub(1))
        };

        assert_eq!(target_idx, total_items - 1);

        let clicked_y = 0;
        let position_ratio = f64::from(clicked_y) / f64::from(scrollbar_height.saturating_sub(1));
        let target_idx = ((position_ratio * (total_items.saturating_sub(1)) as f64) as usize)
            .min(total_items.saturating_sub(1));

        assert_eq!(target_idx, 0);

        let clicked_y = 5;
        let position_ratio = f64::from(clicked_y) / f64::from(scrollbar_height.saturating_sub(1));
        let target_idx = ((position_ratio * (total_items.saturating_sub(1)) as f64) as usize)
            .min(total_items.saturating_sub(1));

        // should be roughly in the middle (around 25-27)
        assert!((20..=30).contains(&target_idx));
    }
}
