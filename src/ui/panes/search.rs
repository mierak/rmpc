use std::rc::Rc;

use anyhow::Context;
use anyhow::Result;
use crossterm::event::KeyCode;
use itertools::Itertools;
use ratatui::layout::Alignment;
use ratatui::layout::Rect;
use ratatui::style::Styled;
use ratatui::widgets::Padding;
use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, Borders, List, ListItem},
};

use crate::cli::create_env;
use crate::cli::run_external;
use crate::config::keys::GlobalAction;
use crate::config::Config;
use crate::config::Search;
use crate::context::AppContext;
use crate::mpd::commands::Song;
use crate::shared::key_event::KeyEvent;
use crate::shared::macros::status_info;
use crate::shared::macros::status_warn;
use crate::shared::mouse_event::MouseEvent;
use crate::shared::mouse_event::MouseEventKind;
use crate::ui::dirstack::Dir;
use crate::ui::UiEvent;
use crate::{
    mpd::mpd_client::{Filter, FilterKind, MpdClient, Tag},
    ui::widgets::{button::Button, input::Input},
};

use super::{CommonAction, Pane};

#[derive(Debug)]
pub struct SearchPane {
    inputs: InputGroups<2, 1>,
    phase: Phase,
    preview: Option<Vec<ListItem<'static>>>,
    songs_dir: Dir<Song>,
    input_areas: Rc<[Rect]>,
    column_areas: [Rect; 3],
}

impl SearchPane {
    pub fn new(context: &AppContext) -> Self {
        let config = context.config;
        Self {
            preview: None,
            phase: Phase::Search,
            songs_dir: Dir::default(),
            inputs: InputGroups::new(
                &config.search,
                [
                    FilterInput {
                        label: " Search mode     :",
                        variant: FilterInputVariant::SelectFilterKind {
                            value: config.search.mode,
                        },
                    },
                    FilterInput {
                        label: " Case Sensistive :",
                        variant: FilterInputVariant::SelectFilterCaseSensitive {
                            value: config.search.case_sensitive,
                        },
                    },
                ],
                [ButtonInput {
                    label: " Reset",
                    variant: ButtonInputVariant::Reset,
                }],
            ),
            input_areas: Rc::default(),
            column_areas: [Rect::default(); 3],
        }
    }

    fn add_current(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        if !self.songs_dir.marked().is_empty() {
            for idx in self.songs_dir.marked() {
                let item = &self.songs_dir.items[*idx];
                client.add(&item.file)?;
            }
            status_info!("Added {} songs queue", self.songs_dir.marked().len());

            context.render()?;
        } else if let Some(item) = self.songs_dir.selected() {
            client.add(&item.file)?;
            status_info!("Added '{}' to queue", item.file);

            context.render()?;
        }

        Ok(())
    }

    fn render_song_column(
        &mut self,
        frame: &mut ratatui::prelude::Frame<'_>,
        area: ratatui::prelude::Rect,
        config: &Config,
    ) {
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
            b.padding(Padding::new(0, 2, 0, 0))
        };
        let current = List::new(self.songs_dir.to_list_items(config)).highlight_style(config.theme.current_item_style);
        let directory = &mut self.songs_dir;

        directory.state.set_content_len(Some(directory.items.len()));
        directory.state.set_viewport_len(Some(area.height.into()));
        if !directory.items.is_empty() && directory.state.get_selected().is_none() {
            directory.state.select(Some(0), 0);
        }
        let area = Rect {
            x: area.x,
            y: area.y,
            width: area.width + 1,
            height: area.height,
        };
        let inner_block = block.inner(area);

        self.column_areas[1] = inner_block;
        frame.render_widget(block, area);
        frame.render_stateful_widget(current, inner_block, directory.state.as_render_state_ref());
        frame.render_stateful_widget(
            config.as_styled_scrollbar(),
            area,
            directory.state.as_scrollbar_state_ref(),
        );
    }

    fn prepare_preview(
        &mut self,
        client: &mut impl MpdClient,
        config: &Config,
    ) -> Result<Option<Vec<ListItem<'static>>>> {
        match &self.phase {
            Phase::SearchTextboxInput => Ok(None),
            Phase::Search => Ok(Some(self.songs_dir.to_list_items(config))),
            Phase::BrowseResults { .. } => {
                let Some(current) = self.songs_dir.selected() else {
                    return Ok(None);
                };

                let preview = client
                    .find(&[Filter::new(Tag::File, &current.file)])?
                    .first()
                    .context("Expected to find exactly one song")?
                    .to_preview(&config.theme.symbols)
                    .collect_vec();
                Ok(Some(preview))
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
                Textbox {
                    value,
                    label,
                    filter_key,
                } => {
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
            Block::default()
                .borders(Borders::TOP)
                .border_style(config.theme.borders_style),
            input_areas[idx],
        );
        idx += 1;

        for input in &self.inputs.filter_inputs {
            let mut inp = match input.variant {
                FilterInputVariant::SelectFilterKind { value } => Input::default()
                    .set_borderless(true)
                    .set_label_style(config.as_text_style())
                    .set_input_style(config.as_text_style())
                    .set_label(input.label)
                    .set_text(Into::into(&value)),
                FilterInputVariant::SelectFilterCaseSensitive { value } => Input::default()
                    .set_borderless(true)
                    .set_label_style(config.as_text_style())
                    .set_input_style(config.as_text_style())
                    .set_label(input.label)
                    .set_text(if value { "Yes" } else { "No" }),
            };

            let is_focused = matches!(self.inputs.focused(),
                FocusedInputGroup::Filters(FilterInput { variant: variant2, .. }) if &input.variant == variant2);

            if is_focused {
                inp = inp
                    .set_label_style(config.theme.current_item_style)
                    .set_input_style(config.theme.current_item_style);
            };
            frame.render_widget(inp, input_areas[idx]);
            idx += 1;
        }

        frame.render_widget(
            Block::default()
                .borders(Borders::TOP)
                .border_style(config.theme.borders_style),
            input_areas[idx],
        );
        idx += 1;

        for input in &self.inputs.button_inputs {
            let mut button = match input.variant {
                ButtonInputVariant::Reset => Button::default().label(input.label).label_alignment(Alignment::Left),
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
        self.inputs
            .filter_inputs
            .iter()
            .fold((FilterKind::Contains, false), |mut acc, val| {
                match val.variant {
                    FilterInputVariant::SelectFilterKind { value } => {
                        acc.0 = value;
                    }
                    FilterInputVariant::SelectFilterCaseSensitive { value } => {
                        acc.1 = value;
                    }
                };
                acc
            })
    }

    fn search_add(&mut self, client: &mut impl MpdClient) -> Result<()> {
        let (filter_kind, case_sensitive) = self.filter_type();
        let filter = self.inputs.textbox_inputs.iter().filter_map(|input| match &input {
            Textbox { value, filter_key, .. } if !value.is_empty() => {
                Some(Filter::new(*filter_key, value).with_type(filter_kind))
            }
            _ => None,
        });

        let filter = filter.collect_vec();

        if filter.is_empty() {
            return Ok(());
        }

        if case_sensitive {
            client.find_add(&filter)?;
        } else {
            client.search_add(&filter)?;
        }

        Ok(())
    }

    fn search(&mut self, client: &mut impl MpdClient) -> Result<Vec<Song>> {
        let (filter_kind, case_sensitive) = self.filter_type();
        let filter = self.inputs.textbox_inputs.iter().filter_map(|input| match &input {
            Textbox { value, filter_key, .. } if !value.is_empty() => {
                Some(Filter::new(*filter_key, value).with_type(filter_kind))
            }
            _ => None,
        });

        let filter = filter.collect_vec();

        if filter.is_empty() {
            return Ok(Vec::new());
        }

        Ok(if case_sensitive {
            client.find(&filter)?
        } else {
            client.search(&filter)?
        })
    }

    fn reset(&mut self, search_config: &Search) {
        for val in &mut self.inputs.textbox_inputs {
            let Textbox { value, .. } = val;
            value.clear();
        }
        for val in &mut self.inputs.filter_inputs {
            match val.variant {
                FilterInputVariant::SelectFilterKind { ref mut value } => *value = search_config.mode,
                FilterInputVariant::SelectFilterCaseSensitive { ref mut value } => {
                    *value = search_config.case_sensitive;
                }
            }
        }
    }

    fn activate_input(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        match self.inputs.focused_mut() {
            FocusedInputGroup::Textboxes(_) => self.phase = Phase::SearchTextboxInput,
            FocusedInputGroup::Buttons(_) => {
                // Reset is the only button in this group at the moment
                self.reset(&context.config.search);
                self.songs_dir = Dir::default();
                self.preview = self.prepare_preview(client, context.config)?;
            }
            FocusedInputGroup::Filters(FilterInput {
                variant: FilterInputVariant::SelectFilterKind { ref mut value },
                ..
            }) => {
                value.cycle();
                self.songs_dir = Dir::new(self.search(client)?);
                self.preview = self.prepare_preview(client, context.config)?;
            }
            FocusedInputGroup::Filters(FilterInput {
                variant: FilterInputVariant::SelectFilterCaseSensitive { ref mut value },
                ..
            }) => {
                *value = !*value;
                self.songs_dir = Dir::new(self.search(client)?);
                self.preview = self.prepare_preview(client, context.config)?;
            }
        };
        Ok(())
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

        // have to account for the separator between filter config inputs/buttons
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
            Block::default()
                .borders(Borders::RIGHT)
                .border_style(config.theme.borders_style),
            previous_area,
        );
        frame.render_widget(
            Block::default()
                .borders(Borders::RIGHT)
                .border_style(config.theme.borders_style),
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
                    frame.render_widget(
                        List::new(preview.to_vec()).highlight_style(config.theme.current_item_style),
                        preview_area,
                    );
                }
            }
            Phase::BrowseResults { filter_input_on: _ } => {
                self.render_song_column(frame, current_area, config);
                self.render_input_column(frame, previous_area, config);
                if let Some(preview) = &self.preview {
                    let preview = List::new(preview.clone()).highlight_style(config.theme.current_item_style);
                    frame.render_widget(preview, preview_area);
                }
            }
        }

        self.column_areas[0] = previous_area;
        self.column_areas[2] = preview_area;

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        if let crate::ui::UiEvent::Database = event {
            self.songs_dir = Dir::default();
            self.preview = self.prepare_preview(client, context.config)?;
            self.phase = Phase::Search;

            status_warn!("The music database has been updated. The current tab has been reinitialized in the root directory to prevent inconsistent behaviours.");
        }

        Ok(())
    }

    fn handle_mouse_event(
        &mut self,
        mut event: MouseEvent,
        client: &mut impl MpdClient,
        context: &mut AppContext,
    ) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick if self.column_areas[0].contains(event.into()) => {
                self.phase = Phase::Search;
                // Modify x coord to belong to middle column in order to satisfy the condition
                // inside get_clicked_input. This is fine because phase is switched to Search.
                // A bit hacky, but wcyd.
                event.x = self.input_areas[1].x;
                if let Some(input) = self.get_clicked_input(event) {
                    self.inputs.focused_idx = input;
                }
                self.preview = self.prepare_preview(client, context.config)?;

                context.render()?;
            }
            MouseEventKind::LeftClick if self.column_areas[2].contains(event.into()) => match self.phase {
                Phase::SearchTextboxInput | Phase::Search => {
                    if !self.songs_dir.items.is_empty() {
                        self.phase = Phase::BrowseResults { filter_input_on: false };

                        let clicked_row: usize = event.y.saturating_sub(self.column_areas[2].y).into();
                        if let Some(idx_to_select) = self.songs_dir.state.get_at_rendered_row(clicked_row) {
                            self.songs_dir
                                .state
                                .set_viewport_len(Some(self.column_areas[2].height as usize));
                            self.songs_dir.select_idx(idx_to_select, context.config.scrolloff);
                        }

                        self.preview = self.prepare_preview(client, context.config)?;

                        context.render()?;
                    }
                }
                Phase::BrowseResults { .. } => {
                    self.add_current(client, context)?;
                }
            },
            MouseEventKind::LeftClick if self.column_areas[1].contains(event.into()) => match self.phase {
                Phase::SearchTextboxInput | Phase::Search => {
                    if matches!(self.phase, Phase::SearchTextboxInput) {
                        self.phase = Phase::Search;
                        self.songs_dir = Dir::new(self.search(client)?);
                        self.preview = self.prepare_preview(client, context.config)?;
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
                        self.preview = self.prepare_preview(client, context.config)?;

                        context.render()?;
                    }
                }
            },
            MouseEventKind::DoubleClick => match self.phase {
                Phase::SearchTextboxInput | Phase::Search => {
                    if self.get_clicked_input(event).is_some() {
                        self.activate_input(client, context)?;

                        context.render()?;
                    }
                }
                Phase::BrowseResults { .. } => {
                    self.add_current(client, context)?;
                }
            },
            MouseEventKind::ScrollDown => match self.phase {
                Phase::SearchTextboxInput | Phase::Search => {
                    if matches!(self.phase, Phase::SearchTextboxInput) {
                        self.phase = Phase::Search;
                        self.songs_dir = Dir::new(self.search(client)?);
                        self.preview = self.prepare_preview(client, context.config)?;
                    }
                    self.inputs.next_non_wrapping();

                    context.render()?;
                }
                Phase::BrowseResults { .. } => {
                    self.songs_dir.next(context.config.scrolloff, false);

                    context.render()?;
                }
            },
            MouseEventKind::ScrollUp => match self.phase {
                Phase::SearchTextboxInput | Phase::Search => {
                    if matches!(self.phase, Phase::SearchTextboxInput) {
                        self.phase = Phase::Search;
                        self.songs_dir = Dir::new(self.search(client)?);
                        self.preview = self.prepare_preview(client, context.config)?;
                    }

                    self.inputs.prev_non_wrapping();

                    context.render()?;
                }
                Phase::BrowseResults { .. } => {
                    self.songs_dir.prev(context.config.scrolloff, false);

                    context.render()?;
                }
            },
            _ => {}
        };

        Ok(())
    }

    fn handle_action(&mut self, event: &mut KeyEvent, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        let config = context.config;
        match &mut self.phase {
            Phase::SearchTextboxInput => match event.as_common_action(context) {
                Some(CommonAction::Close) => {
                    self.phase = Phase::Search;
                    self.songs_dir = Dir::new(self.search(client)?);
                    self.preview = self.prepare_preview(client, config)?;

                    context.render()?;
                }
                Some(CommonAction::Confirm) => {
                    self.phase = Phase::Search;
                    self.songs_dir = Dir::new(self.search(client)?);
                    self.preview = self.prepare_preview(client, config)?;

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
                        run_external(command, create_env(context, songs, client)?);
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
                        CommonAction::Right if !self.songs_dir.items.is_empty() => {
                            self.phase = Phase::BrowseResults { filter_input_on: false };
                            self.preview = self.prepare_preview(client, config)?;

                            context.render()?;
                        }
                        CommonAction::Right => {
                            // TODO Check if needed to render
                        }
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
                        CommonAction::Rename => {}
                        CommonAction::Close => {}
                        CommonAction::Confirm => {
                            self.activate_input(client, context)?;
                            context.render()?;
                        }
                        CommonAction::FocusInput
                            if matches!(self.inputs.focused(), FocusedInputGroup::Textboxes(_)) =>
                        {
                            self.phase = Phase::SearchTextboxInput;

                            context.render()?;
                        }
                        CommonAction::AddAll => {
                            self.search_add(client)?;

                            status_info!("All found songs added to queue");

                            context.render()?;
                        }
                        CommonAction::FocusInput => {}
                        CommonAction::Add => {}
                        CommonAction::Delete => match self.inputs.focused_mut() {
                            FocusedInputGroup::Textboxes(textbox) if !textbox.value.is_empty() => {
                                textbox.value.clear();
                                self.songs_dir = Dir::new(self.search(client)?);
                                self.preview = self.prepare_preview(client, config)?;

                                context.render()?;
                            }
                            _ => {}
                        },
                        CommonAction::PaneDown => {}
                        CommonAction::PaneUp => {}
                        CommonAction::PaneRight => {}
                        CommonAction::PaneLeft => {}
                    }
                }
            }
            Phase::BrowseResults {
                filter_input_on: filter_input_on @ true,
            } => match event.as_common_action(context) {
                Some(CommonAction::Close) => {
                    *filter_input_on = false;
                    self.songs_dir.set_filter(None, config);
                    self.preview = self.prepare_preview(client, config)?;

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
                            self.preview = self.prepare_preview(client, config)?;

                            context.render()?;
                        }
                        KeyCode::Backspace => {
                            self.songs_dir.pop_filter(config);

                            context.render()?;
                        }
                        _ => {}
                    }
                }
            },
            Phase::BrowseResults {
                filter_input_on: filter_input_modce @ false,
            } => {
                if let Some(action) = event.as_global_action(context) {
                    match action {
                        GlobalAction::ExternalCommand { command, .. } if !self.songs_dir.marked().is_empty() => {
                            let songs = self.songs_dir.marked_items().map(|song| song.file.as_str());
                            run_external(command, create_env(context, songs, client)?);
                        }
                        GlobalAction::ExternalCommand { command, .. } => {
                            let selected = self.songs_dir.selected().map(|s| s.file.as_str());
                            run_external(command, create_env(context, selected, client)?);
                        }
                        _ => {
                            event.abandon();
                        }
                    }
                } else if let Some(action) = event.as_common_action(context) {
                    match action {
                        CommonAction::Down => {
                            self.songs_dir
                                .next(context.config.scrolloff, context.config.wrap_navigation);
                            self.preview = self.prepare_preview(client, config)?;

                            context.render()?;
                        }
                        CommonAction::Up => {
                            self.songs_dir
                                .prev(context.config.scrolloff, context.config.wrap_navigation);
                            self.preview = self.prepare_preview(client, config)?;

                            context.render()?;
                        }
                        CommonAction::MoveDown => {}
                        CommonAction::MoveUp => {}
                        CommonAction::DownHalf => {
                            self.songs_dir.next_half_viewport(context.config.scrolloff);
                            self.preview = self.prepare_preview(client, config)?;

                            context.render()?;
                        }
                        CommonAction::UpHalf => {
                            self.songs_dir.prev_half_viewport(context.config.scrolloff);
                            self.preview = self.prepare_preview(client, config)?;

                            context.render()?;
                        }
                        CommonAction::Right => self.add_current(client, context)?,
                        CommonAction::Left => {
                            self.phase = Phase::Search;
                            self.preview = self.prepare_preview(client, config)?;

                            context.render()?;
                        }
                        CommonAction::Top => {
                            self.songs_dir.first();
                            self.preview = self.prepare_preview(client, config)?;

                            context.render()?;
                        }
                        CommonAction::Bottom => {
                            self.songs_dir.last();
                            self.preview = self.prepare_preview(client, config)?;

                            context.render()?;
                        }
                        CommonAction::EnterSearch => {
                            self.songs_dir.set_filter(Some(String::new()), config);
                            *filter_input_modce = true;

                            context.render()?;
                        }
                        CommonAction::NextResult => {
                            self.songs_dir.jump_next_matching(config);
                            self.preview = self.prepare_preview(client, config)?;

                            context.render()?;
                        }
                        CommonAction::PreviousResult => {
                            self.songs_dir.jump_previous_matching(config);
                            self.preview = self.prepare_preview(client, config)?;

                            context.render()?;
                        }
                        CommonAction::Select => {
                            self.songs_dir.toggle_mark_selected();
                            self.songs_dir
                                .next(context.config.scrolloff, context.config.wrap_navigation);

                            context.render()?;
                        }
                        CommonAction::Rename => {}
                        CommonAction::Close => {}
                        CommonAction::Confirm => {
                            self.add_current(client, context)?;

                            context.render()?;
                        }
                        CommonAction::FocusInput => {}
                        CommonAction::Add => self.add_current(client, context)?,
                        CommonAction::AddAll => {
                            self.search_add(client)?;
                            status_info!("All found songs added to queue");

                            context.render()?;
                        }
                        CommonAction::Delete => {}
                        CommonAction::PaneDown => {}
                        CommonAction::PaneUp => {}
                        CommonAction::PaneRight => {}
                        CommonAction::PaneLeft => {}
                    }
                }
            }
        };
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
    pub fn new(search_config: &Search, filter_inputs: [FilterInput; N2], button_inputs: [ButtonInput; N3]) -> Self {
        Self {
            textbox_inputs: search_config
                .tags
                .iter()
                .map(|tag| Textbox {
                    filter_key: tag.value,
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

    pub fn focused_mut(&mut self) -> FocusedInputGroup<&mut Textbox, &mut FilterInput, &mut ButtonInput> {
        match self.focused_idx {
            FocusedInput::Textboxes(idx) => FocusedInputGroup::Textboxes(&mut self.textbox_inputs[idx]),
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
    filter_key: &'static str,
}

#[derive(Debug)]
struct FilterInput {
    variant: FilterInputVariant,
    label: &'static str,
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
