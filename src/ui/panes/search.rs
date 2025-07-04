use std::rc::Rc;

use anyhow::{Context, Result};
use crossterm::event::KeyCode;
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
        keys::{GlobalAction, actions::AddKind},
        tabs::PaneType,
    },
    context::AppContext,
    core::command::{create_env, run_external},
    mpd::{
        QueuePosition,
        commands::Song,
        mpd_client::{Filter, FilterKind, MpdClient, Tag},
    },
    shared::{
        ext::mpd_client::{Enqueue, MpdClientExt},
        key_event::KeyEvent,
        macros::{modal, status_info, status_warn},
        mouse_event::{MouseEvent, MouseEventKind},
        mpd_query::PreviewGroup,
    },
    ui::{
        UiEvent,
        dirstack::{Dir, DirStackItem},
        modals::menu_modal::MenuModal,
        widgets::{button::Button, input::Input},
    },
};

#[derive(Debug)]
pub struct SearchPane {
    inputs: InputGroups<2, 1>,
    phase: Phase,
    preview: Option<Vec<PreviewGroup>>,
    songs_dir: Dir<Song>,
    input_areas: Rc<[Rect]>,
    column_areas: [Rect; 3],
}

const PREVIEW: &str = "preview";
const SEARCH: &str = "search";

impl SearchPane {
    pub fn new(context: &AppContext) -> Self {
        let config = &context.config;
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
            column_areas: [Rect::default(); 3],
        }
    }

    fn add_current(&mut self) -> Vec<Enqueue> {
        if !self.songs_dir.marked().is_empty() {
            self.songs_dir
                .marked()
                .iter()
                .map(|idx| self.songs_dir.items[*idx].file.clone())
                .map(|path| Enqueue::File { path })
                .collect_vec()
        // status_info!("Added {} songs to queue",
        // self.songs_dir.marked().len());
        } else if let Some(item) = self.songs_dir.selected() {
            vec![Enqueue::File { path: item.file.clone() }]
        // status_info!("Added '{item}' to queue");
        } else {
            Vec::new()
        }
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

        self.column_areas[1] = inner_block;
        frame.render_widget(block, area);
        frame.render_stateful_widget(current, inner_block, directory.state.as_render_state_ref());
        if let Some(scrollbar) = config.as_styled_scrollbar() {
            frame.render_stateful_widget(scrollbar, area, directory.state.as_scrollbar_state_ref());
        }
    }

    fn prepare_preview(&mut self, context: &AppContext) {
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
                    self.songs_dir.to_list_items(&context.config),
                )]);
                context.query().id(PREVIEW).replace_id("preview").target(PaneType::Search).query(
                    |_| Ok(MpdQueryResult::Preview { data, origin_path: Some(origin_path) }),
                );
            }
            Phase::BrowseResults { .. } => {
                let Some(current) = self.songs_dir.selected() else {
                    return;
                };
                let file = current.file.clone();
                let key_style = context.config.theme.preview_label_style;
                let group_style = context.config.theme.preview_metadata_group_style;

                context.query().id(PREVIEW).replace_id("preview").target(PaneType::Search).query(
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

    fn search_add(&mut self, context: &AppContext, position: Option<QueuePosition>) {
        let (filter_kind, case_sensitive) = self.filter_type();
        let filter = self.inputs.textbox_inputs.iter().filter_map(|input| match &input {
            Textbox { value, filter_key, .. } if !value.is_empty() => {
                Some((filter_key.to_owned(), value.to_owned(), filter_kind))
            }
            _ => None,
        });

        let mut filter = filter.collect_vec();

        if filter.is_empty() {
            return;
        }

        if case_sensitive {
            context.command(move |client| {
                client.find_add(
                    &filter
                        .iter_mut()
                        .map(|&mut (ref mut key, ref value, ref mut kind)| {
                            Filter::new(std::mem::take(key), value).with_type(*kind)
                        })
                        .collect_vec(),
                    position,
                )?;
                Ok(())
            });
        } else {
            context.command(move |client| {
                client.search_add(
                    &filter
                        .iter_mut()
                        .map(|&mut (ref mut key, ref value, ref mut kind)| {
                            Filter::new(std::mem::take(key), value).with_type(*kind)
                        })
                        .collect_vec(),
                    position,
                )?;
                Ok(())
            });
        }
    }

    fn search(&mut self, context: &AppContext) {
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

        context.query().id(SEARCH).replace_id(SEARCH).target(PaneType::Search).query(
            move |client| {
                let filter = filter
                    .iter_mut()
                    .map(|&mut (ref mut key, ref value, ref mut kind)| {
                        Filter::new(std::mem::take(key), value).with_type(*kind)
                    })
                    .collect_vec();
                let result =
                    if case_sensitive { client.find(&filter) } else { client.search(&filter) }?;

                Ok(MpdQueryResult::SongsList { data: result, origin_path: None })
            },
        );
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

    fn activate_input(&mut self, context: &AppContext) {
        match self.inputs.focused_mut() {
            FocusedInputGroup::Textboxes(_) => self.phase = Phase::SearchTextboxInput,
            FocusedInputGroup::Buttons(_) => {
                // Reset is the only button in this group at the moment
                self.reset(&context.config.search);
                self.songs_dir = Dir::default();
                self.prepare_preview(context);
            }
            FocusedInputGroup::Filters(FilterInput {
                variant: FilterInputVariant::SelectFilterKind { value },
                ..
            }) => {
                value.cycle();
                self.search(context);
            }
            FocusedInputGroup::Filters(FilterInput {
                variant: FilterInputVariant::SelectFilterCaseSensitive { value },
                ..
            }) => {
                *value = !*value;
                self.search(context);
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
}

impl Pane for SearchPane {
    fn render(
        &mut self,
        frame: &mut ratatui::prelude::Frame,
        area: ratatui::prelude::Rect,
        AppContext { config, .. }: &AppContext,
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
                self.column_areas[1] = current_area;
                self.render_input_column(frame, current_area, config);

                // Render preview at offset to allow click to select
                if let Some(preview) = &self
                    .preview
                    .as_ref()
                    .and_then(|preview| preview.get(self.songs_dir.state.offset()..))
                {
                    let mut result = Vec::new();
                    for group in *preview {
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

        self.column_areas[0] = previous_area;
        self.column_areas[2] = preview_area;

        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut UiEvent,
        _is_visible: bool,
        context: &AppContext,
    ) -> Result<()> {
        match event {
            UiEvent::Database => {
                self.songs_dir = Dir::default();
                self.prepare_preview(context);
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
                *self = Self::new(context);
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
        context: &AppContext,
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
                context.render()?;
            }
            (SEARCH, MpdQueryResult::SongsList { data, origin_path: _ }) => {
                self.songs_dir = Dir::new(data);
                self.preview = Some(vec![PreviewGroup::from(
                    None,
                    None,
                    self.songs_dir.to_list_items(&context.config),
                )]);
                context.render()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, mut event: MouseEvent, context: &AppContext) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick if self.column_areas[0].contains(event.into()) => {
                self.phase = Phase::Search;
                // Modify x coord to belong to middle column in order to satisfy
                // the condition inside get_clicked_input. This
                // is fine because phase is switched to Search.
                // A bit hacky, but wcyd.
                event.x = self.input_areas[1].x;
                if let Some(input) = self.get_clicked_input(event) {
                    self.inputs.focused_idx = input;
                }
                self.prepare_preview(context);

                context.render()?;
            }
            MouseEventKind::LeftClick if self.column_areas[2].contains(event.into()) => {
                match self.phase {
                    Phase::SearchTextboxInput | Phase::Search => {
                        if !self.songs_dir.items.is_empty() {
                            self.phase = Phase::BrowseResults { filter_input_on: false };

                            let clicked_row: usize =
                                event.y.saturating_sub(self.column_areas[2].y).into();
                            if let Some(idx_to_select) =
                                self.songs_dir.state.get_at_rendered_row(clicked_row)
                            {
                                self.songs_dir
                                    .state
                                    .set_viewport_len(Some(self.column_areas[2].height as usize));
                                self.songs_dir.select_idx(idx_to_select, context.config.scrolloff);
                            }

                            self.prepare_preview(context);

                            context.render()?;
                        }
                    }
                    Phase::BrowseResults { .. } => {
                        let items = self.add_current();
                        if !items.is_empty() {
                            context.command(move |client| {
                                client.send_enqueue_multiple(items, None)?;
                                Ok(())
                            });
                        }
                    }
                }
            }
            MouseEventKind::LeftClick if self.column_areas[1].contains(event.into()) => {
                match self.phase {
                    Phase::SearchTextboxInput | Phase::Search => {
                        if matches!(self.phase, Phase::SearchTextboxInput) {
                            self.phase = Phase::Search;
                            self.search(context);
                        }

                        if let Some(input) = self.get_clicked_input(event) {
                            self.inputs.focused_idx = input;
                        }

                        context.render()?;
                    }
                    Phase::BrowseResults { .. } => {
                        let clicked_row = event.y.saturating_sub(self.column_areas[1].y).into();
                        if let Some(idx) = self.songs_dir.state.get_at_rendered_row(clicked_row) {
                            self.songs_dir.select_idx(idx, context.config.scrolloff);
                            self.prepare_preview(context);

                            context.render()?;
                        }
                    }
                }
            }
            MouseEventKind::DoubleClick => match self.phase {
                Phase::SearchTextboxInput | Phase::Search => {
                    if self.get_clicked_input(event).is_some() {
                        self.activate_input(context);

                        context.render()?;
                    }
                }
                Phase::BrowseResults { .. } => {
                    let items = self.add_current();
                    if !items.is_empty() {
                        context.command(move |client| {
                            client.send_enqueue_multiple(items, None)?;
                            Ok(())
                        });
                    }
                }
            },
            MouseEventKind::MiddleClick if self.column_areas[1].contains(event.into()) => {
                match self.phase {
                    Phase::SearchTextboxInput | Phase::Search => {}
                    Phase::BrowseResults { .. } => {
                        let clicked_row = event.y.saturating_sub(self.column_areas[1].y).into();
                        if let Some(idx) = self.songs_dir.state.get_at_rendered_row(clicked_row) {
                            self.songs_dir.select_idx(idx, context.config.scrolloff);
                            self.prepare_preview(context);

                            self.songs_dir.select_idx(idx, context.config.scrolloff);
                            if let Some(item) = self.songs_dir.selected() {
                                let item = item.file.clone();
                                context.command(move |client| {
                                    client.add(&item, None)?;
                                    status_info!("Added '{item}' to queue");
                                    Ok(())
                                });
                            }
                            self.prepare_preview(context);
                            context.render()?;
                        }
                    }
                }
            }
            MouseEventKind::ScrollDown => match self.phase {
                Phase::SearchTextboxInput | Phase::Search => {
                    if matches!(self.phase, Phase::SearchTextboxInput) {
                        self.phase = Phase::Search;
                        self.search(context);
                    }
                    self.inputs.next_non_wrapping();

                    context.render()?;
                }
                Phase::BrowseResults { .. } => {
                    self.songs_dir.next(context.config.scrolloff, false);
                    self.prepare_preview(context);

                    context.render()?;
                }
            },
            MouseEventKind::ScrollUp => match self.phase {
                Phase::SearchTextboxInput | Phase::Search => {
                    if matches!(self.phase, Phase::SearchTextboxInput) {
                        self.phase = Phase::Search;
                        self.search(context);
                    }

                    self.inputs.prev_non_wrapping();

                    context.render()?;
                }
                Phase::BrowseResults { .. } => {
                    self.songs_dir.prev(context.config.scrolloff, false);
                    self.prepare_preview(context);

                    context.render()?;
                }
            },
            _ => {}
        }

        Ok(())
    }

    fn handle_action(&mut self, event: &mut KeyEvent, context: &mut AppContext) -> Result<()> {
        let config = &context.config;
        match &mut self.phase {
            Phase::SearchTextboxInput => match event.as_common_action(context) {
                Some(CommonAction::Close) => {
                    self.phase = Phase::Search;
                    self.search(context);

                    context.render()?;
                }
                Some(CommonAction::Confirm) => {
                    self.phase = Phase::Search;
                    self.search(context);

                    context.render()?;
                }
                _ => {
                    event.stop_propagation();
                    match event.code() {
                        KeyCode::Char(c) => match self.inputs.focused_mut() {
                            FocusedInputGroup::Textboxes(Textbox { value, .. }) => {
                                value.push(c);

                                context.render()?;
                            }
                            FocusedInputGroup::Filters(_) | FocusedInputGroup::Buttons(_) => {}
                        },
                        KeyCode::Backspace => match self.inputs.focused_mut() {
                            FocusedInputGroup::Textboxes(Textbox { value, .. }) => {
                                value.pop();

                                context.render()?;
                            }
                            FocusedInputGroup::Filters(_) | FocusedInputGroup::Buttons(_) => {}
                        },
                        _ => {}
                    }
                }
            },
            Phase::Search => {
                if let Some(action) = event.as_global_action(context) {
                    if let GlobalAction::ExternalCommand { command, .. } = action {
                        let songs = self.songs_dir.items.iter().map(|song| song.file.as_str());
                        run_external(command.clone(), create_env(context, songs));
                    } else {
                        event.abandon();
                    }
                } else if let Some(action) = event.as_common_action(context) {
                    match action {
                        CommonAction::Down => {
                            if config.wrap_navigation {
                                self.inputs.next();
                            } else {
                                self.inputs.next_non_wrapping();
                            }

                            context.render()?;
                        }
                        CommonAction::Up => {
                            if config.wrap_navigation {
                                self.inputs.prev();
                            } else {
                                self.inputs.prev_non_wrapping();
                            }

                            context.render()?;
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
                            self.prepare_preview(context);

                            context.render()?;
                        }
                        CommonAction::Right => {}
                        CommonAction::Left => {}
                        CommonAction::Top => {
                            self.inputs.first();

                            context.render()?;
                        }
                        CommonAction::Bottom => {
                            self.inputs.last();

                            context.render()?;
                        }
                        CommonAction::EnterSearch => {}
                        CommonAction::NextResult => {}
                        CommonAction::PreviousResult => {}
                        CommonAction::Select => {}
                        CommonAction::InvertSelection => {}
                        CommonAction::Rename => {}
                        CommonAction::Close => {}
                        CommonAction::Confirm => {
                            self.activate_input(context);
                            context.render()?;
                        }
                        CommonAction::FocusInput
                            if matches!(self.inputs.focused(), FocusedInputGroup::Textboxes(_)) =>
                        {
                            self.phase = Phase::SearchTextboxInput;

                            context.render()?;
                        }
                        CommonAction::AddAll => {
                            self.search_add(context, None);

                            status_info!("All found songs added to queue");

                            context.render()?;
                        }
                        CommonAction::InsertAll => {
                            self.search_add(context, Some(QueuePosition::RelativeAdd(0)));

                            status_info!("All found songs added to queue");

                            context.render()?;
                        }
                        CommonAction::AddAllReplace => {
                            context.command(|client| {
                                client.clear()?;
                                Ok(())
                            });
                            self.search_add(context, None);

                            status_info!("All found songs added to queue");

                            context.render()?;
                        }
                        CommonAction::FocusInput => {}
                        CommonAction::Delete => match self.inputs.focused_mut() {
                            FocusedInputGroup::Textboxes(textbox) if !textbox.value.is_empty() => {
                                textbox.value.clear();
                                self.search(context);

                                context.render()?;
                            }
                            _ => {}
                        },
                        CommonAction::PaneDown => {}
                        CommonAction::PaneUp => {}
                        CommonAction::PaneRight => {}
                        CommonAction::PaneLeft => {}
                        CommonAction::ShowInfo => {}
                        CommonAction::AddOptions { .. } => {}
                    }
                }
            }
            Phase::BrowseResults { filter_input_on: filter_input_on @ true } => {
                match event.as_common_action(context) {
                    Some(CommonAction::Close) => {
                        *filter_input_on = false;
                        self.songs_dir.set_filter(None, config);
                        self.prepare_preview(context);

                        context.render()?;
                    }
                    Some(CommonAction::Confirm) => {
                        *filter_input_on = false;

                        context.render()?;
                    }
                    _ => {
                        event.stop_propagation();
                        match event.code() {
                            KeyCode::Char(c) => {
                                self.songs_dir.push_filter(c, config);
                                self.songs_dir.jump_first_matching(config);
                                self.prepare_preview(context);

                                context.render()?;
                            }
                            KeyCode::Backspace => {
                                self.songs_dir.pop_filter(config);

                                context.render()?;
                            }
                            _ => {}
                        }
                    }
                }
            }
            Phase::BrowseResults { filter_input_on: filter_input_mode @ false } => {
                if let Some(action) = event.as_global_action(context) {
                    match action {
                        GlobalAction::ExternalCommand { command, .. }
                            if !self.songs_dir.marked().is_empty() =>
                        {
                            let songs =
                                self.songs_dir.marked_items().map(|song| song.file.as_str());
                            run_external(command.clone(), create_env(context, songs));
                        }
                        GlobalAction::ExternalCommand { command, .. } => {
                            let selected = self.songs_dir.selected().map(|s| s.file.as_str());
                            run_external(command.clone(), create_env(context, selected));
                        }
                        _ => {
                            event.abandon();
                        }
                    }
                } else if let Some(action) = event.as_common_action(context) {
                    match action.to_owned() {
                        CommonAction::Down => {
                            self.songs_dir
                                .next(context.config.scrolloff, context.config.wrap_navigation);
                            self.prepare_preview(context);

                            context.render()?;
                        }
                        CommonAction::Up => {
                            self.songs_dir
                                .prev(context.config.scrolloff, context.config.wrap_navigation);
                            self.prepare_preview(context);

                            context.render()?;
                        }
                        CommonAction::MoveDown => {}
                        CommonAction::MoveUp => {}
                        CommonAction::DownHalf => {
                            self.songs_dir.next_half_viewport(context.config.scrolloff);
                            self.prepare_preview(context);

                            context.render()?;
                        }
                        CommonAction::UpHalf => {
                            self.songs_dir.prev_half_viewport(context.config.scrolloff);
                            self.prepare_preview(context);

                            context.render()?;
                        }
                        CommonAction::PageDown => {
                            self.songs_dir.next_viewport(context.config.scrolloff);
                            self.prepare_preview(context);

                            context.render()?;
                        }
                        CommonAction::PageUp => {
                            self.songs_dir.prev_viewport(context.config.scrolloff);
                            self.prepare_preview(context);

                            context.render()?;
                        }
                        CommonAction::Right => {
                            let items = self.add_current();
                            if !items.is_empty() {
                                context.command(move |client| {
                                    client.send_enqueue_multiple(items, None)?;
                                    Ok(())
                                });
                            }
                        }
                        CommonAction::Left => {
                            self.phase = Phase::Search;
                            self.prepare_preview(context);

                            context.render()?;
                        }
                        CommonAction::Top => {
                            self.songs_dir.first();
                            self.prepare_preview(context);

                            context.render()?;
                        }
                        CommonAction::Bottom => {
                            self.songs_dir.last();
                            self.prepare_preview(context);

                            context.render()?;
                        }
                        CommonAction::EnterSearch => {
                            self.songs_dir.set_filter(Some(String::new()), config);
                            *filter_input_mode = true;

                            context.render()?;
                        }
                        CommonAction::NextResult => {
                            self.songs_dir.jump_next_matching(config);
                            self.prepare_preview(context);

                            context.render()?;
                        }
                        CommonAction::PreviousResult => {
                            self.songs_dir.jump_previous_matching(config);
                            self.prepare_preview(context);

                            context.render()?;
                        }
                        CommonAction::Select => {
                            self.songs_dir.toggle_mark_selected();
                            self.songs_dir
                                .next(context.config.scrolloff, context.config.wrap_navigation);

                            context.render()?;
                        }
                        CommonAction::InvertSelection => {
                            self.songs_dir.invert_marked();

                            context.render()?;
                        }
                        CommonAction::Close if !self.songs_dir.marked().is_empty() => {
                            self.songs_dir.marked_mut().clear();
                            context.render()?;
                        }
                        CommonAction::Rename => {}
                        CommonAction::Close => {}
                        CommonAction::Confirm => {
                            let items = self.add_current();
                            let queue_len = context.queue.len();
                            if !items.is_empty() {
                                context.command(move |client| {
                                    client.send_enqueue_multiple(items, None)?;
                                    client.play_position_safe(queue_len)?;
                                    Ok(())
                                });
                            }

                            context.render()?;
                        }
                        CommonAction::FocusInput => {}
                        CommonAction::AddAll => {
                            self.search_add(context, None);
                            status_info!("All found songs added to queue");

                            context.render()?;
                        }
                        CommonAction::AddOptions { kind: AddKind::Action(opts) } => {
                            let items = self.add_current();
                            let queue_len = context.queue.len();
                            let current_song_idx =
                                context.find_current_song_in_queue().map(|(i, _)| i);

                            if !items.is_empty() {
                                context.command(move |client| {
                                    if opts.replace {
                                        client.clear()?;
                                    }

                                    let position = opts.to_queue_position();
                                    let play_pos_idx =
                                        opts.play_position_idx(queue_len, current_song_idx);

                                    client.send_enqueue_multiple(items, position)?;

                                    if let Some(pos) = play_pos_idx {
                                        client.play_position_safe(pos)?;
                                    }

                                    Ok(())
                                });
                            }
                        }
                        CommonAction::AddOptions { kind: AddKind::Modal(opts) } => {
                            let opts = opts
                                .iter()
                                .map(|(label, opts)| (label.to_owned(), *opts, self.add_current()))
                                .collect_vec();

                            modal!(context, MenuModal::create_add_modal(opts, context));
                        }
                        CommonAction::InsertAll => {
                            self.search_add(context, Some(QueuePosition::RelativeAdd(0)));
                            status_info!("All found songs added to queue");

                            context.render()?;
                        }
                        CommonAction::AddAllReplace => {
                            context.command(|client| {
                                client.clear()?;
                                Ok(())
                            });
                            self.search_add(context, None);
                            status_info!("All found songs added to queue");

                            context.render()?;
                        }
                        CommonAction::Delete => {}
                        CommonAction::PaneDown => {}
                        CommonAction::PaneUp => {}
                        CommonAction::PaneRight => {}
                        CommonAction::PaneLeft => {}
                        CommonAction::ShowInfo => {}
                    }
                }
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
