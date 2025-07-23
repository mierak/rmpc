use std::rc::Rc;

use anyhow::{Context, Result};
use crossterm::event::KeyCode;
use enum_map::EnumMap;
use itertools::Itertools;
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Styled, Stylize},
    text::Span,
    widgets::{Block, Borders, List, ListItem, Padding},
};

use super::{CommonAction, Pane};
use crate::{
    MpdQueryResult,
    config::{
        Config,
        Search,
        keys::{
            GlobalAction,
            actions::{AddKind, Position},
        },
        tabs::PaneType,
    },
    core::command::{create_env, run_external},
    ctx::Ctx,
    mpd::{
        commands::Song,
        mpd_client::{Filter, FilterKind, MpdClient, Tag},
    },
    shared::{
        key_event::KeyEvent,
        macros::{modal, status_info, status_warn},
        mouse_event::{MouseEvent, MouseEventKind},
        mpd_client_ext::{Autoplay, Enqueue, MpdClientExt},
        mpd_query::PreviewGroup,
    },
    ui::{
        UiEvent,
        dirstack::{Dir, DirStackItem},
        modals::{
            input_modal::InputModal,
            menu::{create_add_modal, modal::MenuModal},
            select_modal::SelectModal,
        },
        widgets::{browser::BrowserArea, button::Button, input::Input},
    },
};

#[derive(Debug)]
pub struct SearchPane {
    inputs: InputGroups<2, 1>,
    phase: Phase,
    preview: Option<Vec<PreviewGroup>>,
    songs_dir: Dir<Song>,
    input_areas: Rc<[Rect]>,
    column_areas: EnumMap<BrowserArea, Rect>,
}

const PREVIEW: &str = "preview";
const SEARCH: &str = "search";

impl SearchPane {
    pub fn new(ctx: &Ctx) -> Self {
        let config = &ctx.config;
        Self {
            preview: None,
            phase: Phase::Search,
            songs_dir: Dir::default(),
            inputs: InputGroups::new(
                &config.search,
                [
                    FilterInput {
                        label: " Search mode     :".to_string(),
                        variant: FilterInputVariant::SelectFilterKind { value: config.search.mode },
                    },
                    FilterInput {
                        label: " Case sensitive  :".to_string(),
                        variant: FilterInputVariant::SelectFilterCaseSensitive {
                            value: config.search.case_sensitive,
                        },
                    },
                ],
                [ButtonInput { label: " Reset", variant: ButtonInputVariant::Reset }],
            ),
            input_areas: Rc::default(),
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
        config: &Config,
    ) {
        let column_right_padding: u16 = config.theme.scrollbar.is_some().into();
        let title = self.songs_dir.filter().as_ref().map(|v| {
            format!(
                "[FILTER]: {v}{} ",
                if matches!(self.phase, Phase::BrowseResults { filter_input_on: true }) {
                    "█"
                } else {
                    ""
                }
            )
        });

        let block = {
            let mut b = Block::default();
            if let Some(ref title) = title {
                b = b.title(title.clone().set_style(config.theme.borders_style));
            }
            b.padding(Padding::new(0, column_right_padding, 0, 0))
        };
        let current = List::new(self.songs_dir.to_list_items(config))
            .highlight_style(config.theme.current_item_style);
        let directory = &mut self.songs_dir;

        directory.state.set_content_len(Some(directory.items.len()));
        directory.state.set_viewport_len(Some(area.height.into()));
        if !directory.items.is_empty() && directory.state.get_selected().is_none() {
            directory.state.select(Some(0), 0);
        }
        let inner_block = block.inner(area);

        self.column_areas[BrowserArea::Current] = inner_block;
        self.column_areas[BrowserArea::Scrollbar] =
            if matches!(self.phase, Phase::BrowseResults { .. }) { area } else { Rect::default() };
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

    fn prepare_preview(&mut self, ctx: &Ctx) {
        let Some(origin_path) = self.songs_dir.selected().map(|s| vec![s.as_path().to_owned()])
        else {
            return;
        };
        match &self.phase {
            Phase::SearchTextboxInput => {}
            Phase::Search => {
                let data = Some(vec![PreviewGroup::from(
                    None,
                    None,
                    self.songs_dir.to_list_items(&ctx.config),
                )]);
                ctx.query().id(PREVIEW).replace_id("preview").target(PaneType::Search).query(
                    |_| Ok(MpdQueryResult::Preview { data, origin_path: Some(origin_path) }),
                );
            }
            Phase::BrowseResults { .. } => {
                let Some(current) = self.songs_dir.selected() else {
                    return;
                };
                let file = current.file.clone();
                let key_style = ctx.config.theme.preview_label_style;
                let group_style = ctx.config.theme.preview_metadata_group_style;

                ctx.query().id(PREVIEW).replace_id("preview").target(PaneType::Search).query(
                    move |client| {
                        let data = Some(
                            client
                                .find(&[Filter::new(Tag::File, &file)])?
                                .first()
                                .context("Expected to find exactly one song")?
                                .to_preview(key_style, group_style),
                        );
                        Ok(MpdQueryResult::Preview { data, origin_path: Some(origin_path) })
                    },
                );
            }
        }
    }

    fn render_input_column(
        &mut self,
        frame: &mut ratatui::prelude::Frame,
        area: ratatui::prelude::Rect,
        config: &Config,
    ) {
        let input_areas = Layout::vertical(
            (0..self.inputs.textbox_inputs.len()
                + self.inputs.filter_inputs.len()
                + self.inputs.button_inputs.len()
                + 2) // +2 for borders/separators
                .map(|_| Constraint::Length(1)),
        )
        .split(area);

        self.input_areas = Rc::clone(&input_areas);

        let mut idx = 0;
        for input in &self.inputs.textbox_inputs {
            match input {
                Textbox { value, label, filter_key } => {
                    let is_focused = matches!(self.inputs.focused(),
                        FocusedInputGroup::Textboxes(Textbox { filter_key: filter_key2, .. }) if filter_key == filter_key2);

                    let mut widget = Input::default()
                        .set_borderless(true)
                        .set_label(label)
                        .set_placeholder("<None>")
                        .set_focused(is_focused && matches!(self.phase, Phase::SearchTextboxInput))
                        .set_label_style(config.as_text_style())
                        .set_input_style(config.as_text_style())
                        .set_text(value);

                    widget = if matches!(self.phase, Phase::SearchTextboxInput) && is_focused {
                        widget.set_label_style(config.theme.highlighted_item_style)
                    } else if is_focused {
                        widget
                            .set_label_style(config.theme.current_item_style)
                            .set_input_style(config.theme.current_item_style)
                    } else if !value.is_empty() {
                        widget.set_input_style(config.theme.highlighted_item_style)
                    } else {
                        widget
                    };

                    frame.render_widget(widget, input_areas[idx]);
                }
            }
            idx += 1;
        }

        frame.render_widget(
            Block::default().borders(Borders::TOP).border_style(config.theme.borders_style),
            input_areas[idx],
        );
        idx += 1;

        for input in &self.inputs.filter_inputs {
            let mut inp = match input.variant {
                FilterInputVariant::SelectFilterKind { value } => Input::default()
                    .set_borderless(true)
                    .set_label_style(config.as_text_style())
                    .set_input_style(config.as_text_style())
                    .set_label(&input.label)
                    .set_text(Into::into(&value)),
                FilterInputVariant::SelectFilterCaseSensitive { value } => Input::default()
                    .set_borderless(true)
                    .set_label_style(config.as_text_style())
                    .set_input_style(config.as_text_style())
                    .set_label(&input.label)
                    .set_text(if value { "Yes" } else { "No" }),
            };

            let is_focused = matches!(self.inputs.focused(),
                FocusedInputGroup::Filters(FilterInput { variant: variant2, .. }) if &input.variant == variant2);

            if is_focused {
                inp = inp
                    .set_label_style(config.theme.current_item_style)
                    .set_input_style(config.theme.current_item_style);
            }
            frame.render_widget(inp, input_areas[idx]);
            idx += 1;
        }

        frame.render_widget(
            Block::default().borders(Borders::TOP).border_style(config.theme.borders_style),
            input_areas[idx],
        );
        idx += 1;

        for input in &self.inputs.button_inputs {
            let mut button = match input.variant {
                ButtonInputVariant::Reset => {
                    Button::default().label(input.label).label_alignment(Alignment::Left)
                }
            };

            let is_focused = matches!(self.inputs.focused(),
                FocusedInputGroup::Buttons(ButtonInput { variant, .. }) if &input.variant == variant);

            if is_focused {
                button = button.style(config.theme.current_item_style);
            } else {
                button = button.style(config.as_text_style());
            }
            frame.render_widget(button, input_areas[idx]);
        }
    }

    fn filter_type(&self) -> (FilterKind, bool) {
        self.inputs.filter_inputs.iter().fold((FilterKind::Contains, false), |mut acc, val| {
            match val.variant {
                FilterInputVariant::SelectFilterKind { value } => {
                    acc.0 = value;
                }
                FilterInputVariant::SelectFilterCaseSensitive { value } => {
                    acc.1 = value;
                }
            }
            acc
        })
    }

    fn search(&mut self, ctx: &Ctx) {
        let (filter_kind, case_sensitive) = self.filter_type();
        let filter = self.inputs.textbox_inputs.iter().filter_map(|input| match &input {
            Textbox { value, filter_key, .. } if !value.is_empty() => {
                Some((filter_key.to_owned(), value.to_owned(), filter_kind))
            }
            _ => None,
        });

        let mut filter = filter.collect_vec();

        if filter.is_empty() {
            let _ = std::mem::take(&mut self.songs_dir);
            self.preview.take();
            return;
        }

        ctx.query().id(SEARCH).replace_id(SEARCH).target(PaneType::Search).query(move |client| {
            let filter = filter
                .iter_mut()
                .map(|&mut (ref mut key, ref value, ref mut kind)| {
                    Filter::new(std::mem::take(key), value).with_type(*kind)
                })
                .collect_vec();
            let result =
                if case_sensitive { client.find(&filter) } else { client.search(&filter) }?;

            Ok(MpdQueryResult::SongsList { data: result, origin_path: None })
        });
    }

    fn reset(&mut self, search_config: &Search) {
        for val in &mut self.inputs.textbox_inputs {
            let Textbox { value, .. } = val;
            value.clear();
        }
        for val in &mut self.inputs.filter_inputs {
            match val.variant {
                FilterInputVariant::SelectFilterKind { ref mut value } => {
                    *value = search_config.mode;
                }
                FilterInputVariant::SelectFilterCaseSensitive { ref mut value } => {
                    *value = search_config.case_sensitive;
                }
            }
        }
    }

    fn activate_input(&mut self, ctx: &Ctx) {
        match self.inputs.focused_mut() {
            FocusedInputGroup::Textboxes(_) => self.phase = Phase::SearchTextboxInput,
            FocusedInputGroup::Buttons(_) => {
                // Reset is the only button in this group at the moment
                self.reset(&ctx.config.search);
                self.songs_dir = Dir::default();
                self.prepare_preview(ctx);
            }
            FocusedInputGroup::Filters(FilterInput {
                variant: FilterInputVariant::SelectFilterKind { value },
                ..
            }) => {
                value.cycle();
                self.search(ctx);
            }
            FocusedInputGroup::Filters(FilterInput {
                variant: FilterInputVariant::SelectFilterCaseSensitive { value },
                ..
            }) => {
                *value = !*value;
                self.search(ctx);
            }
        }
    }

    fn get_clicked_input(&self, event: MouseEvent) -> Option<FocusedInput> {
        for i in 0..self.inputs.textbox_inputs.len() {
            if self.input_areas[i].contains(event.into()) {
                return Some(FocusedInput::Textboxes(i));
            }
        }

        // have to account for the separator between inputs/filter config inputs
        let start = self.inputs.textbox_inputs.len() + 1;
        for i in start..start + self.inputs.filter_inputs.len() {
            if self.input_areas[i].contains(event.into()) {
                return Some(FocusedInput::Filters(i - start));
            }
        }

        // have to account for the separator between filter config
        // inputs/buttons
        let start = start + self.inputs.filter_inputs.len() + 1;
        for i in start..start + self.inputs.button_inputs.len() {
            if self.input_areas[i].contains(event.into()) {
                return Some(FocusedInput::Buttons(i - start));
            }
        }

        None
    }

    fn handle_search_phase_action(&mut self, event: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        let config = &ctx.config;
        if let Some(action) = event.as_global_action(ctx) {
            if let GlobalAction::ExternalCommand { command, .. } = action {
                let songs = self.songs_dir.items.iter().map(|song| song.file.as_str());
                run_external(command.clone(), create_env(ctx, songs));
            } else {
                event.abandon();
            }
        } else if let Some(action) = event.as_common_action(ctx) {
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
                    self.phase = Phase::BrowseResults { filter_input_on: false };
                    self.preview = None;
                    self.prepare_preview(ctx);

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
                    self.activate_input(ctx);
                    ctx.render()?;
                }
                CommonAction::FocusInput
                    if matches!(self.inputs.focused(), FocusedInputGroup::Textboxes(_)) =>
                {
                    self.phase = Phase::SearchTextboxInput;

                    ctx.render()?;
                }
                // Modal while we are on search column does not support all options. It can
                // be implemented later.
                CommonAction::AddOptions { kind: AddKind::Modal(_) } => {}
                CommonAction::AddOptions { kind: AddKind::Action(opts) } if opts.all => {
                    let (_, enqueue) = self.enqueue(opts.all);
                    if !enqueue.is_empty() {
                        let queue_len = ctx.queue.len();
                        let current_song_idx = ctx.find_current_song_in_queue().map(|(i, _)| i);

                        ctx.command(move |client| {
                            let autoplay = opts.autoplay(queue_len, current_song_idx, None);
                            client.enqueue_multiple(enqueue, opts.position, autoplay)?;

                            Ok(())
                        });
                    }
                }
                // This action only makes sense when opts.all is true while we are on the
                // search column.
                CommonAction::AddOptions { kind: AddKind::Action(_) } => {}
                CommonAction::FocusInput => {}
                CommonAction::Delete => match self.inputs.focused_mut() {
                    FocusedInputGroup::Textboxes(textbox) if !textbox.value.is_empty() => {
                        textbox.value.clear();
                        self.search(ctx);

                        ctx.render()?;
                    }
                    _ => {}
                },
                CommonAction::PaneDown => {}
                CommonAction::PaneUp => {}
                CommonAction::PaneRight => {}
                CommonAction::PaneLeft => {}
                CommonAction::ShowInfo => {}
                CommonAction::ContextMenu => {}
            }
        }

        Ok(())
    }

    fn handle_result_phase_search(&mut self, event: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        let Phase::BrowseResults { filter_input_on } = &mut self.phase else {
            return Ok(());
        };
        let config = &ctx.config;
        match event.as_common_action(ctx) {
            Some(CommonAction::Close) => {
                *filter_input_on = false;
                self.songs_dir.set_filter(None, config);
                self.prepare_preview(ctx);

                ctx.render()?;
            }
            Some(CommonAction::Confirm) => {
                *filter_input_on = false;

                ctx.render()?;
            }
            _ => {
                event.stop_propagation();
                match event.code() {
                    KeyCode::Char(c) => {
                        self.songs_dir.push_filter(c, config);
                        self.songs_dir.jump_first_matching(config);
                        self.prepare_preview(ctx);

                        ctx.render()?;
                    }
                    KeyCode::Backspace => {
                        self.songs_dir.pop_filter(config);

                        ctx.render()?;
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn handle_result_phase_action(&mut self, event: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        let Phase::BrowseResults { filter_input_on } = &mut self.phase else {
            return Ok(());
        };
        let config = &ctx.config;
        if let Some(action) = event.as_global_action(ctx) {
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
        } else if let Some(action) = event.as_common_action(ctx) {
            match action.to_owned() {
                CommonAction::Down => {
                    self.songs_dir.next(ctx.config.scrolloff, ctx.config.wrap_navigation);
                    self.prepare_preview(ctx);

                    ctx.render()?;
                }
                CommonAction::Up => {
                    self.songs_dir.prev(ctx.config.scrolloff, ctx.config.wrap_navigation);
                    self.prepare_preview(ctx);

                    ctx.render()?;
                }
                CommonAction::MoveDown => {}
                CommonAction::MoveUp => {}
                CommonAction::DownHalf => {
                    self.songs_dir.next_half_viewport(ctx.config.scrolloff);
                    self.prepare_preview(ctx);

                    ctx.render()?;
                }
                CommonAction::UpHalf => {
                    self.songs_dir.prev_half_viewport(ctx.config.scrolloff);
                    self.prepare_preview(ctx);

                    ctx.render()?;
                }
                CommonAction::PageDown => {
                    self.songs_dir.next_viewport(ctx.config.scrolloff);
                    self.prepare_preview(ctx);

                    ctx.render()?;
                }
                CommonAction::PageUp => {
                    self.songs_dir.prev_viewport(ctx.config.scrolloff);
                    self.prepare_preview(ctx);

                    ctx.render()?;
                }
                CommonAction::Right => {
                    let items = self.songs_dir.selected().map_or_else(Vec::new, |item| {
                        vec![Enqueue::File { path: item.file.clone() }]
                    });
                    if !items.is_empty() {
                        ctx.command(move |client| {
                            client.enqueue_multiple(items, Position::EndOfQueue, Autoplay::None)?;
                            Ok(())
                        });
                    }
                }
                CommonAction::Left => {
                    self.phase = Phase::Search;
                    self.prepare_preview(ctx);

                    ctx.render()?;
                }
                CommonAction::Top => {
                    self.songs_dir.first();
                    self.prepare_preview(ctx);

                    ctx.render()?;
                }
                CommonAction::Bottom => {
                    self.songs_dir.last();
                    self.prepare_preview(ctx);

                    ctx.render()?;
                }
                CommonAction::EnterSearch => {
                    self.songs_dir.set_filter(Some(String::new()), config);
                    *filter_input_on = true;

                    ctx.render()?;
                }
                CommonAction::NextResult => {
                    self.songs_dir.jump_next_matching(config);
                    self.prepare_preview(ctx);

                    ctx.render()?;
                }
                CommonAction::PreviousResult => {
                    self.songs_dir.jump_previous_matching(config);
                    self.prepare_preview(ctx);

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
                    let queue_len = ctx.queue.len();
                    let current_song_idx = ctx.find_current_song_in_queue().map(|(i, _)| i);

                    if !items.is_empty() {
                        ctx.command(move |client| {
                            client.enqueue_multiple(
                                items,
                                Position::Replace,
                                Autoplay::Hovered { queue_len, current_song_idx, hovered_song_idx },
                            )?;
                            Ok(())
                        });
                    }

                    ctx.render()?;
                }
                CommonAction::Confirm => {}
                CommonAction::FocusInput => {}
                CommonAction::AddOptions { kind: AddKind::Action(opts) } => {
                    let (hovered_song_idx, enqueue) = self.enqueue(opts.all);

                    if !enqueue.is_empty() {
                        let queue_len = ctx.queue.len();
                        let current_song_idx = ctx.find_current_song_in_queue().map(|(i, _)| i);

                        ctx.command(move |client| {
                            let autoplay =
                                opts.autoplay(queue_len, current_song_idx, hovered_song_idx);

                            client.enqueue_multiple(enqueue, opts.position, autoplay)?;

                            Ok(())
                        });
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
                    self.open_result_phase_context_menu(ctx)?;
                }
            }
        }

        Ok(())
    }

    fn open_result_phase_context_menu(&self, ctx: &Ctx) -> Result<()> {
        let modal = MenuModal::new(ctx)
            .list_section(ctx, move |mut section| {
                if !self.songs_dir.items.is_empty() {
                    let (_, enqueue) = self.enqueue(true);
                    if !enqueue.is_empty() {
                        let enqueue_clone = enqueue.clone();
                        section.add_item("Add all to queue", move |ctx| {
                            ctx.command(move |client| {
                                client.enqueue_multiple(
                                    enqueue_clone,
                                    Position::EndOfQueue,
                                    Autoplay::None,
                                )?;
                                Ok(())
                            });
                            Ok(())
                        });
                        section.add_item("Replace queue with all", move |ctx| {
                            ctx.command(move |client| {
                                client.enqueue_multiple(
                                    enqueue,
                                    Position::Replace,
                                    Autoplay::None,
                                )?;
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
        Ok(())
    }

    fn scrollbar_area(&self) -> Option<Rect> {
        let area = self.column_areas[BrowserArea::Scrollbar];
        if area.width > 0 { Some(area) } else { None }
    }

    fn handle_scrollbar_interaction(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<bool> {
        if !matches!(self.phase, Phase::BrowseResults { .. }) {
            return Ok(false);
        }
        let Some(_) = ctx.config.theme.scrollbar else {
            return Ok(false);
        };
        let Some(scrollbar_area) = self.scrollbar_area() else {
            return Ok(false);
        };
        match event.kind {
            MouseEventKind::LeftClick | MouseEventKind::Drag { .. } => {
                if crate::shared::mouse_event::is_scrollbar_interaction(event, scrollbar_area) {
                    let content_len = self.songs_dir.items.len();
                    if let Some(target_idx) = crate::shared::mouse_event::calculate_scrollbar_index(
                        event,
                        scrollbar_area,
                        content_len,
                    ) {
                        self.songs_dir.select_idx(target_idx, ctx.config.scrolloff);
                        self.prepare_preview(ctx);
                        ctx.render()?;
                        return Ok(true);
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_search_pane_phase_check() {
        assert!(matches!(
            Phase::BrowseResults { filter_input_on: false },
            Phase::BrowseResults { .. }
        ));
        assert!(!matches!(Phase::Search, Phase::BrowseResults { .. }));
        assert!(!matches!(Phase::SearchTextboxInput, Phase::BrowseResults { .. }));
    }
}

impl Pane for SearchPane {
    fn render(
        &mut self,
        frame: &mut ratatui::prelude::Frame,
        area: ratatui::prelude::Rect,
        Ctx { config, .. }: &Ctx,
    ) -> anyhow::Result<()> {
        let widths = &config.theme.column_widths;
        let [previous_area, current_area_init, preview_area] = *Layout::horizontal([
            Constraint::Percentage(widths[0]),
            Constraint::Percentage(widths[1]),
            Constraint::Percentage(widths[2]),
        ])
        .split(area) else {
            return Ok(());
        };

        frame.render_widget(
            Block::default().borders(Borders::RIGHT).border_style(config.theme.borders_style),
            previous_area,
        );
        frame.render_widget(
            Block::default().borders(Borders::RIGHT).border_style(config.theme.borders_style),
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
            Phase::Search | Phase::SearchTextboxInput => {
                self.column_areas[BrowserArea::Current] = current_area;
                self.render_input_column(frame, current_area, config);

                // Render preview at offset to allow click to select
                if let Some(preview) = &self.preview {
                    let offset = self.songs_dir.state.offset();
                    let mut skipped = 0;
                    let mut result = Vec::new();
                    for group in preview {
                        if let Some(name) = group.name {
                            // TODO color should be corrected
                            result.push(ListItem::new(name).yellow().bold());
                        }
                        let mut added_any = false;
                        if skipped < offset {
                            result.extend(group.items.iter().skip(offset - skipped).cloned());
                            skipped += offset - skipped;
                            added_any = true;
                        } else {
                            result.extend(group.items.clone());
                        }
                        if added_any {
                            result.push(ListItem::new(Span::raw("")));
                        }
                    }
                    let preview = List::new(result).style(config.as_text_style());
                    frame.render_widget(preview, preview_area);
                }
            }
            Phase::BrowseResults { filter_input_on: _ } => {
                self.render_song_column(frame, current_area, config);
                self.render_input_column(frame, previous_area, config);
                if let Some(preview) = &self.preview {
                    let mut result = Vec::new();
                    for group in preview {
                        if let Some(name) = group.name {
                            result.push(ListItem::new(name).yellow().bold());
                        }
                        result.extend(group.items.clone());
                        result.push(ListItem::new(Span::raw("")));
                    }
                    let preview = List::new(result).style(config.as_text_style());
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
                self.prepare_preview(ctx);
                self.phase = Phase::Search;

                status_warn!(
                    "The music database has been updated. The current tab has been reinitialized in the root directory to prevent inconsistent behaviours."
                );
            }
            UiEvent::Reconnected => {
                self.phase = Phase::Search;
                self.preview = None;
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
            (PREVIEW, MpdQueryResult::Preview { data, origin_path }) => {
                let Some(selected) = self.songs_dir.selected().map(|s| [s.as_path()]) else {
                    log::trace!("Dropping preview because no item was selected");
                    return Ok(());
                };
                if let Some(origin_path) = origin_path {
                    if origin_path != selected {
                        log::trace!(origin_path:?, current_path:? = selected; "Dropping preview because it does not belong to this path");
                        return Ok(());
                    }
                }
                self.preview = data;
                ctx.render()?;
            }
            (SEARCH, MpdQueryResult::SongsList { data, origin_path: _ }) => {
                self.songs_dir = Dir::new(data);
                self.preview = Some(vec![PreviewGroup::from(
                    None,
                    None,
                    self.songs_dir.to_list_items(&ctx.config),
                )]);
                ctx.render()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, mut event: MouseEvent, ctx: &Ctx) -> Result<()> {
        if self.handle_scrollbar_interaction(event, ctx)? {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::LeftClick
                if self.column_areas[BrowserArea::Previous].contains(event.into()) =>
            {
                self.phase = Phase::Search;
                // Modify x coord to belong to middle column in order to satisfy
                // the condition inside get_clicked_input. This
                // is fine because phase is switched to Search.
                // A bit hacky, but wcyd.
                event.x = self.input_areas[1].x;
                if let Some(input) = self.get_clicked_input(event) {
                    self.inputs.focused_idx = input;
                }
                self.prepare_preview(ctx);

                ctx.render()?;
            }
            MouseEventKind::LeftClick
                if self.column_areas[BrowserArea::Preview].contains(event.into()) =>
            {
                match self.phase {
                    Phase::SearchTextboxInput | Phase::Search => {
                        if !self.songs_dir.items.is_empty() {
                            self.phase = Phase::BrowseResults { filter_input_on: false };

                            let clicked_row: usize = event
                                .y
                                .saturating_sub(self.column_areas[BrowserArea::Preview].y)
                                .into();
                            if let Some(idx_to_select) =
                                self.songs_dir.state.get_at_rendered_row(clicked_row)
                            {
                                self.songs_dir.state.set_viewport_len(Some(
                                    self.column_areas[BrowserArea::Preview].height as usize,
                                ));
                                self.songs_dir.select_idx(idx_to_select, ctx.config.scrolloff);
                            }

                            self.prepare_preview(ctx);

                            ctx.render()?;
                        }
                    }
                    Phase::BrowseResults { .. } => {
                        let (_, items) = self.enqueue(false);
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
                }
            }
            MouseEventKind::LeftClick
                if self.column_areas[BrowserArea::Current].contains(event.into()) =>
            {
                match self.phase {
                    Phase::SearchTextboxInput | Phase::Search => {
                        if matches!(self.phase, Phase::SearchTextboxInput) {
                            self.phase = Phase::Search;
                            self.search(ctx);
                        }

                        if let Some(input) = self.get_clicked_input(event) {
                            self.inputs.focused_idx = input;
                        }

                        ctx.render()?;
                    }
                    Phase::BrowseResults { .. } => {
                        let clicked_row = event
                            .y
                            .saturating_sub(self.column_areas[BrowserArea::Current].y)
                            .into();

                        if let Some(idx) = self.songs_dir.state.get_at_rendered_row(clicked_row) {
                            self.songs_dir.select_idx(idx, ctx.config.scrolloff);

                            self.prepare_preview(ctx);

                            ctx.render()?;
                        }
                    }
                }
            }
            MouseEventKind::DoubleClick => match self.phase {
                Phase::SearchTextboxInput | Phase::Search => {
                    if self.get_clicked_input(event).is_some() {
                        self.activate_input(ctx);
                        ctx.render()?;
                    }
                }
                Phase::BrowseResults { .. } => {
                    let (_, items) = self.enqueue(false);
                    if !items.is_empty() {
                        ctx.command(move |client| {
                            client.enqueue_multiple(items, Position::EndOfQueue, Autoplay::None)?;
                            Ok(())
                        });
                    }
                }
            },
            MouseEventKind::MiddleClick
                if self.column_areas[BrowserArea::Current].contains(event.into()) =>
            {
                match self.phase {
                    Phase::SearchTextboxInput | Phase::Search => {}
                    Phase::BrowseResults { .. } => {
                        let clicked_row = event
                            .y
                            .saturating_sub(self.column_areas[BrowserArea::Current].y)
                            .into();
                        if let Some(idx) = self.songs_dir.state.get_at_rendered_row(clicked_row) {
                            self.songs_dir.select_idx(idx, ctx.config.scrolloff);
                            self.prepare_preview(ctx);
                            self.songs_dir.select_idx(idx, ctx.config.scrolloff);
                            if let Some(item) = self.songs_dir.selected() {
                                let item = item.file.clone();
                                ctx.command(move |client| {
                                    client.add(&item, None)?;
                                    status_info!("Added '{item}' to queue");
                                    Ok(())
                                });
                            }
                            self.prepare_preview(ctx);
                            ctx.render()?;
                        }
                    }
                }
            }
            MouseEventKind::ScrollDown => match self.phase {
                Phase::SearchTextboxInput | Phase::Search => {
                    if matches!(self.phase, Phase::SearchTextboxInput) {
                        self.phase = Phase::Search;
                        self.search(ctx);
                    }
                    self.inputs.next_non_wrapping();
                    ctx.render()?;
                }
                Phase::BrowseResults { .. } => {
                    self.songs_dir.next(ctx.config.scrolloff, false);
                    self.prepare_preview(ctx);
                    ctx.render()?;
                }
            },
            MouseEventKind::ScrollUp => match self.phase {
                Phase::SearchTextboxInput | Phase::Search => {
                    if matches!(self.phase, Phase::SearchTextboxInput) {
                        self.phase = Phase::Search;
                        self.search(ctx);
                    }
                    self.inputs.prev_non_wrapping();
                    ctx.render()?;
                }
                Phase::BrowseResults { .. } => {
                    self.songs_dir.prev(ctx.config.scrolloff, false);
                    self.prepare_preview(ctx);
                    ctx.render()?;
                }
            },
            MouseEventKind::RightClick => match self.phase {
                Phase::BrowseResults { filter_input_on: false } => {
                    let clicked_row =
                        event.y.saturating_sub(self.column_areas[BrowserArea::Current].y).into();
                    if let Some(idx) = self.songs_dir.state.get_at_rendered_row(clicked_row) {
                        self.songs_dir.select_idx(idx, ctx.config.scrolloff);
                        self.prepare_preview(ctx);
                        ctx.render()?;
                    }
                    self.open_result_phase_context_menu(ctx)?;
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

    fn handle_action(&mut self, event: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        match &mut self.phase {
            Phase::SearchTextboxInput => match event.as_common_action(ctx) {
                Some(CommonAction::Close) => {
                    self.phase = Phase::Search;
                    self.search(ctx);

                    ctx.render()?;
                }
                Some(CommonAction::Confirm) => {
                    self.phase = Phase::Search;
                    self.search(ctx);

                    ctx.render()?;
                }
                _ => {
                    event.stop_propagation();
                    match event.code() {
                        KeyCode::Char(c) => match self.inputs.focused_mut() {
                            FocusedInputGroup::Textboxes(Textbox { value, .. }) => {
                                value.push(c);

                                ctx.render()?;
                            }
                            FocusedInputGroup::Filters(_) | FocusedInputGroup::Buttons(_) => {}
                        },
                        KeyCode::Backspace => match self.inputs.focused_mut() {
                            FocusedInputGroup::Textboxes(Textbox { value, .. }) => {
                                value.pop();

                                ctx.render()?;
                            }
                            FocusedInputGroup::Filters(_) | FocusedInputGroup::Buttons(_) => {}
                        },
                        _ => {}
                    }
                }
            },
            Phase::Search => {
                self.handle_search_phase_action(event, ctx)?;
            }
            Phase::BrowseResults { filter_input_on: true } => {
                self.handle_result_phase_search(event, ctx)?;
            }
            Phase::BrowseResults { filter_input_on: false } => {
                self.handle_result_phase_action(event, ctx)?;
            }
        }
        Ok(())
    }
}

enum FocusedInputGroup<T, F, B> {
    Textboxes(T),
    Filters(F),
    Buttons(B),
}

#[derive(Debug)]
enum FocusedInput {
    Textboxes(usize),
    Filters(usize),
    Buttons(usize),
}

#[derive(Debug)]
struct InputGroups<const N2: usize, const N3: usize> {
    textbox_inputs: Vec<Textbox>,
    filter_inputs: [FilterInput; N2],
    button_inputs: [ButtonInput; N3],
    focused_idx: FocusedInput,
}

impl<const N2: usize, const N3: usize> InputGroups<N2, N3> {
    pub fn new(
        search_config: &Search,
        filter_inputs: [FilterInput; N2],
        button_inputs: [ButtonInput; N3],
    ) -> Self {
        Self {
            textbox_inputs: search_config
                .tags
                .iter()
                .map(|tag| Textbox {
                    filter_key: tag.value.clone(),
                    label: format!(" {:<16}:", tag.label),
                    value: String::new(),
                })
                .collect_vec(),
            filter_inputs,
            button_inputs,
            focused_idx: FocusedInput::Textboxes(0),
        }
    }

    pub fn first(&mut self) {
        self.focused_idx = FocusedInput::Textboxes(0);
    }

    pub fn last(&mut self) {
        self.focused_idx = FocusedInput::Buttons(self.button_inputs.len() - 1);
    }

    pub fn focused_mut(
        &mut self,
    ) -> FocusedInputGroup<&mut Textbox, &mut FilterInput, &mut ButtonInput> {
        match self.focused_idx {
            FocusedInput::Textboxes(idx) => {
                FocusedInputGroup::Textboxes(&mut self.textbox_inputs[idx])
            }
            FocusedInput::Filters(idx) => FocusedInputGroup::Filters(&mut self.filter_inputs[idx]),
            FocusedInput::Buttons(idx) => FocusedInputGroup::Buttons(&mut self.button_inputs[idx]),
        }
    }

    pub fn focused(&self) -> FocusedInputGroup<&Textbox, &FilterInput, &ButtonInput> {
        match self.focused_idx {
            FocusedInput::Textboxes(idx) => FocusedInputGroup::Textboxes(&self.textbox_inputs[idx]),
            FocusedInput::Filters(idx) => FocusedInputGroup::Filters(&self.filter_inputs[idx]),
            FocusedInput::Buttons(idx) => FocusedInputGroup::Buttons(&self.button_inputs[idx]),
        }
    }

    pub fn next_non_wrapping(&mut self) {
        match self.focused_idx {
            FocusedInput::Textboxes(idx) if idx == self.textbox_inputs.len() - 1 => {
                self.focused_idx = FocusedInput::Filters(0);
            }
            FocusedInput::Textboxes(ref mut idx) => {
                *idx += 1;
            }
            FocusedInput::Filters(idx) if idx == self.filter_inputs.len() - 1 => {
                self.focused_idx = FocusedInput::Buttons(0);
            }
            FocusedInput::Filters(ref mut idx) => {
                *idx += 1;
            }
            FocusedInput::Buttons(idx) if idx == self.button_inputs.len() - 1 => {}
            FocusedInput::Buttons(ref mut idx) => {
                *idx += 1;
            }
        }
    }

    pub fn next(&mut self) {
        match self.focused_idx {
            FocusedInput::Textboxes(idx) if idx == self.textbox_inputs.len() - 1 => {
                self.focused_idx = FocusedInput::Filters(0);
            }
            FocusedInput::Textboxes(ref mut idx) => {
                *idx += 1;
            }
            FocusedInput::Filters(idx) if idx == self.filter_inputs.len() - 1 => {
                self.focused_idx = FocusedInput::Buttons(0);
            }
            FocusedInput::Filters(ref mut idx) => {
                *idx += 1;
            }
            FocusedInput::Buttons(idx) if idx == self.button_inputs.len() - 1 => {
                self.focused_idx = FocusedInput::Textboxes(0);
            }
            FocusedInput::Buttons(ref mut idx) => {
                *idx += 1;
            }
        }
    }

    pub fn prev_non_wrapping(&mut self) {
        match self.focused_idx {
            FocusedInput::Textboxes(0) => {}
            FocusedInput::Textboxes(ref mut idx) => {
                *idx -= 1;
            }
            FocusedInput::Filters(0) => {
                self.focused_idx = FocusedInput::Textboxes(self.textbox_inputs.len() - 1);
            }
            FocusedInput::Filters(ref mut idx) => {
                *idx -= 1;
            }
            FocusedInput::Buttons(0) => {
                self.focused_idx = FocusedInput::Filters(self.filter_inputs.len() - 1);
            }
            FocusedInput::Buttons(ref mut idx) => {
                *idx -= 1;
            }
        }
    }

    pub fn prev(&mut self) {
        match self.focused_idx {
            FocusedInput::Textboxes(0) => {
                self.focused_idx = FocusedInput::Buttons(self.button_inputs.len() - 1);
            }
            FocusedInput::Textboxes(ref mut idx) => {
                *idx -= 1;
            }
            FocusedInput::Filters(0) => {
                self.focused_idx = FocusedInput::Textboxes(self.textbox_inputs.len() - 1);
            }
            FocusedInput::Filters(ref mut idx) => {
                *idx -= 1;
            }
            FocusedInput::Buttons(0) => {
                self.focused_idx = FocusedInput::Filters(self.filter_inputs.len() - 1);
            }
            FocusedInput::Buttons(ref mut idx) => {
                *idx -= 1;
            }
        }
    }
}

#[derive(Debug)]
enum Phase {
    SearchTextboxInput,
    Search,
    BrowseResults { filter_input_on: bool },
}

#[derive(Debug)]
struct Textbox {
    value: String,
    label: String,
    filter_key: String,
}

#[derive(Debug)]
struct FilterInput {
    variant: FilterInputVariant,
    label: String,
}

#[derive(Debug, PartialEq)]
enum FilterInputVariant {
    SelectFilterKind { value: FilterKind },
    SelectFilterCaseSensitive { value: bool },
}

#[derive(Debug)]
struct ButtonInput {
    variant: ButtonInputVariant,
    label: &'static str,
}

#[derive(Debug, PartialEq)]
enum ButtonInputVariant {
    Reset,
}
