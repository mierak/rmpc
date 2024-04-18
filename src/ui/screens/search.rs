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
use strum::Display;

use crate::config::Config;
use crate::mpd::commands::Song;
use crate::mpd::commands::Status;
use crate::ui::utils::dirstack::Dir;
use crate::{
    mpd::mpd_client::{Filter, FilterKind, MpdClient, Tag},
    ui::{
        widgets::{button::Button, input::Input},
        KeyHandleResultInternal,
    },
};

use super::{CommonAction, Screen};

#[derive(Debug)]
pub struct SearchScreen {
    inputs: InputGroups<7, 2, 1>,
    phase: Phase,
    preview: Option<Vec<ListItem<'static>>>,
    songs_dir: Dir<Song>,
}

impl SearchScreen {
    fn add_current(&mut self, client: &mut impl MpdClient) -> Result<KeyHandleResultInternal> {
        if let Some(item) = self.songs_dir.selected() {
            client.add(&item.file)?;
            Ok(KeyHandleResultInternal::RenderRequested)
        } else {
            Ok(KeyHandleResultInternal::SkipRender)
        }
    }

    fn render_song_column(
        &mut self,
        frame: &mut ratatui::prelude::Frame<'_>,
        area: ratatui::prelude::Rect,
        config: &Config,
    ) {
        let title = self.songs_dir.filter.as_ref().map(|v| format!("[FILTER]: {v} "));
        let current = List::new(self.songs_dir.to_list_items(config))
            .block({
                let mut b = Block::default();
                if let Some(ref title) = title {
                    b = b.title(title.clone().set_style(config.ui.borders_style));
                }
                b.padding(Padding::new(0, 2, 0, 0))
            })
            .highlight_style(config.ui.current_item_style);
        let directory = &mut self.songs_dir;

        directory.state.set_content_len(Some(directory.items.len()));
        directory.state.set_viewport_len(Some(area.height.into()));
        if !directory.items.is_empty() && directory.state.get_selected().is_none() {
            directory.state.select(Some(0));
        }
        let area = Rect {
            x: area.x,
            y: area.y,
            width: area.width + 1,
            height: area.height,
        };
        frame.render_stateful_widget(current, area, directory.state.as_render_state_ref());
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
            Phase::List { .. } => {
                let Some(current) = self.songs_dir.selected() else {
                    return Ok(None);
                };

                let preview = client
                    .find(&[Filter::new(Tag::File, &current.file)])?
                    .first()
                    .context("Expected to find exactly one song")?
                    .to_preview(&config.ui.symbols)
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
                + 2) // +2 for because of separators
                .map(|_| Constraint::Length(1)),
        )
        .split(area);

        let mut idx = 0;
        for input in &self.inputs.textbox_inputs {
            match input {
                Textbox {
                    value, label, variant, ..
                } => {
                    let is_focused = matches!(self.inputs.focused(),
                        FocusedInputGroup::Textboxes(Textbox { variant: variant2, .. }) if variant == variant2);

                    let mut widget = Input::default()
                        .set_borderless(true)
                        .set_label(label)
                        .set_placeholder("<None>")
                        .set_focused(is_focused && matches!(self.phase, Phase::SearchTextboxInput))
                        .set_text(value);

                    widget = if matches!(self.phase, Phase::SearchTextboxInput) && is_focused {
                        widget.set_label_style(config.ui.highlighted_item_style)
                    } else if is_focused {
                        widget
                            .set_label_style(config.ui.current_item_style)
                            .set_input_style(config.ui.current_item_style)
                    } else if !value.is_empty() {
                        widget.set_input_style(config.ui.highlighted_item_style)
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
                .border_style(config.ui.borders_style),
            input_areas[idx],
        );
        idx += 1;

        for input in &self.inputs.filter_inputs {
            let mut inp = match input.variant {
                FilterInputVariant::SelectFilterKind { value } => Input::default()
                    .set_borderless(true)
                    .set_label(input.label)
                    .set_text(Into::into(&value)),
                FilterInputVariant::SelectFilterCaseSensitive { value } => Input::default()
                    .set_borderless(true)
                    .set_label(input.label)
                    .set_text(if value { "Yes" } else { "No" }),
            };

            let is_focused = matches!(self.inputs.focused(),
                FocusedInputGroup::Filters(FilterInput { variant: variant2, .. }) if &input.variant == variant2);

            if is_focused {
                inp = inp
                    .set_label_style(config.ui.current_item_style)
                    .set_input_style(config.ui.current_item_style);
            };
            frame.render_widget(inp, input_areas[idx]);
            idx += 1;
        }

        frame.render_widget(
            Block::default()
                .borders(Borders::TOP)
                .border_style(config.ui.borders_style),
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
                button = button.style(config.ui.current_item_style);
            };
            frame.render_widget(button, input_areas[idx]);
        }
    }

    fn search(&mut self, client: &mut impl MpdClient) -> Result<Vec<Song>> {
        let (filter_kind, case_sensitive) =
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
                });

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

    fn reset(&mut self) {
        for val in &mut self.inputs.textbox_inputs {
            let Textbox { ref mut value, .. } = val;
            value.clear();
        }
        for val in &mut self.inputs.filter_inputs {
            match val.variant {
                FilterInputVariant::SelectFilterKind { ref mut value } => *value = FilterKind::Contains,
                FilterInputVariant::SelectFilterCaseSensitive { ref mut value } => *value = false,
            }
        }
    }
}

impl Screen for SearchScreen {
    type Actions = SearchActions;

    fn render(
        &mut self,
        frame: &mut ratatui::prelude::Frame,
        area: ratatui::prelude::Rect,
        _status: &Status,
        config: &Config,
    ) -> anyhow::Result<()> {
        let widths = &config.ui.column_widths;
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
                .border_style(config.ui.borders_style),
            previous_area,
        );
        frame.render_widget(
            Block::default()
                .borders(Borders::RIGHT)
                .border_style(config.ui.borders_style),
            current_area_init,
        );
        let previous_area = Rect {
            x: previous_area.x,
            y: previous_area.y,
            width: previous_area.width - 1,
            height: previous_area.height,
        };
        let current_area = Rect {
            x: current_area_init.x,
            y: current_area_init.y,
            width: current_area_init.width - 1,
            height: current_area_init.height,
        };

        match self.phase {
            Phase::Search | Phase::SearchTextboxInput => {
                self.render_input_column(frame, current_area, config);
            }
            Phase::List { filter_input_on: _ } => {
                self.render_song_column(frame, current_area, config);
                self.render_input_column(frame, previous_area, config);
            }
        }

        if let Some(preview) = &self.preview {
            let preview = List::new(preview.clone()).highlight_style(config.ui.current_item_style);
            frame.render_widget(preview, preview_area);
        }

        Ok(())
    }

    fn handle_action(
        &mut self,
        event: crossterm::event::KeyEvent,
        client: &mut impl MpdClient,
        _status: &mut Status,
        config: &Config,
    ) -> anyhow::Result<crate::ui::KeyHandleResultInternal> {
        let action = config.keybinds.navigation.get(&event.into());
        match &mut self.phase {
            Phase::SearchTextboxInput => {
                if let Some(CommonAction::Close) = action {
                    self.phase = Phase::Search;
                    self.songs_dir = Dir::new(self.search(client)?);
                    self.preview = self.prepare_preview(client, config)?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                } else if let Some(CommonAction::Confirm) = action {
                    self.phase = Phase::Search;
                    self.songs_dir = Dir::new(self.search(client)?);
                    self.preview = self.prepare_preview(client, config)?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                } else {
                    match event.code {
                        KeyCode::Char(c) => match self.inputs.focused_mut() {
                            FocusedInputGroup::Textboxes(Textbox { value, .. }) => {
                                value.push(c);
                                Ok(KeyHandleResultInternal::RenderRequested)
                            }
                            FocusedInputGroup::Filters(_) | FocusedInputGroup::Buttons(_) => {
                                Ok(KeyHandleResultInternal::SkipRender)
                            }
                        },
                        KeyCode::Backspace => match self.inputs.focused_mut() {
                            FocusedInputGroup::Textboxes(Textbox { value, .. }) => {
                                value.pop();
                                Ok(KeyHandleResultInternal::RenderRequested)
                            }
                            FocusedInputGroup::Filters(_) | FocusedInputGroup::Buttons(_) => {
                                Ok(KeyHandleResultInternal::SkipRender)
                            }
                        },
                        _ => Ok(KeyHandleResultInternal::SkipRender),
                    }
                }
            }
            Phase::Search => {
                if let Some(action) = config.keybinds.navigation.get(&event.into()) {
                    match action {
                        CommonAction::Down => {
                            self.inputs.next();
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::Up => {
                            self.inputs.prev();
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::MoveDown => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::MoveUp => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::DownHalf => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::UpHalf => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::Right if !self.songs_dir.items.is_empty() => {
                            self.phase = Phase::List { filter_input_on: false };
                            self.preview = self.prepare_preview(client, config)?;
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::Right => Ok(KeyHandleResultInternal::RenderRequested),
                        CommonAction::Left => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::Top => {
                            self.inputs.first();
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::Bottom => {
                            self.inputs.last();
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::EnterSearch => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::NextResult => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::PreviousResult => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::Select => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::Rename => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::Close => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::Confirm => {
                            match self.inputs.focused_mut() {
                                FocusedInputGroup::Textboxes(_) => self.phase = Phase::SearchTextboxInput,
                                FocusedInputGroup::Buttons(_) => {
                                    self.reset();
                                    self.songs_dir = Dir::default();
                                    self.preview = self.prepare_preview(client, config)?;
                                }
                                FocusedInputGroup::Filters(FilterInput {
                                    variant: FilterInputVariant::SelectFilterKind { ref mut value },
                                    ..
                                }) => {
                                    value.cycle();
                                    self.songs_dir = Dir::new(self.search(client)?);
                                    self.preview = self.prepare_preview(client, config)?;
                                }
                                FocusedInputGroup::Filters(FilterInput {
                                    variant: FilterInputVariant::SelectFilterCaseSensitive { ref mut value },
                                    ..
                                }) => {
                                    *value = !*value;
                                    self.songs_dir = Dir::new(self.search(client)?);
                                    self.preview = self.prepare_preview(client, config)?;
                                }
                            }
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::FocusInput
                            if matches!(self.inputs.focused(), FocusedInputGroup::Textboxes(_)) =>
                        {
                            self.phase = Phase::SearchTextboxInput;
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::FocusInput => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::Add => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::Delete => Ok(KeyHandleResultInternal::KeyNotHandled),
                    }
                } else {
                    Ok(KeyHandleResultInternal::KeyNotHandled)
                }
            }
            Phase::List {
                filter_input_on: filter_input_on @ true,
            } => {
                if let Some(CommonAction::Close) = action {
                    *filter_input_on = false;
                    self.songs_dir.filter = None;
                    self.preview = self.prepare_preview(client, config)?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                } else if let Some(CommonAction::Confirm) = action {
                    *filter_input_on = false;
                    self.songs_dir.jump_next_matching();
                    self.preview = self.prepare_preview(client, config)?;
                    Ok(KeyHandleResultInternal::RenderRequested)
                } else {
                    match event.code {
                        KeyCode::Char(c) => {
                            if let Some(ref mut f) = self.songs_dir.filter {
                                f.push(c);
                            }
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        KeyCode::Backspace => {
                            if let Some(ref mut f) = self.songs_dir.filter {
                                f.pop();
                            }
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        _ => Ok(KeyHandleResultInternal::SkipRender),
                    }
                }
            }
            Phase::List {
                filter_input_on: filter_input_modce @ false,
            } => {
                if let Some(action) = config.keybinds.navigation.get(&event.into()) {
                    match action {
                        CommonAction::Down => {
                            self.songs_dir.next();
                            self.preview = self.prepare_preview(client, config)?;
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::Up => {
                            self.songs_dir.prev();
                            self.preview = self.prepare_preview(client, config)?;
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::MoveDown => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::MoveUp => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::DownHalf => {
                            self.songs_dir.next_half_viewport();
                            self.preview = self.prepare_preview(client, config)?;
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::UpHalf => {
                            self.songs_dir.prev_half_viewport();
                            self.preview = self.prepare_preview(client, config)?;
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::Right => self.add_current(client),
                        CommonAction::Left => {
                            self.phase = Phase::Search;
                            self.preview = self.prepare_preview(client, config)?;
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::Top => {
                            self.songs_dir.first();
                            self.preview = self.prepare_preview(client, config)?;
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::Bottom => {
                            self.songs_dir.last();
                            self.preview = self.prepare_preview(client, config)?;
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::EnterSearch => {
                            self.songs_dir.filter = Some(String::new());
                            *filter_input_modce = true;
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::NextResult => {
                            self.songs_dir.jump_next_matching();
                            self.preview = self.prepare_preview(client, config)?;
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::PreviousResult => {
                            self.songs_dir.jump_previous_matching();
                            self.preview = self.prepare_preview(client, config)?;
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::Select => {
                            self.songs_dir.toggle_mark_selected();
                            self.songs_dir.next();
                            Ok(KeyHandleResultInternal::RenderRequested)
                        }
                        CommonAction::Rename => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::Close => Ok(KeyHandleResultInternal::KeyNotHandled),
                        CommonAction::Confirm => self.add_current(client),
                        CommonAction::FocusInput => Ok(KeyHandleResultInternal::SkipRender),
                        CommonAction::Add => {
                            if !self.songs_dir.marked().is_empty() {
                                for idx in self.songs_dir.marked() {
                                    let item = &self.songs_dir.items[*idx];
                                    client.add(&item.file)?;
                                }
                                Ok(KeyHandleResultInternal::RenderRequested)
                            } else if let Some(item) = self.songs_dir.selected() {
                                client.add(&item.file)?;
                                Ok(KeyHandleResultInternal::RenderRequested)
                            } else {
                                Ok(KeyHandleResultInternal::SkipRender)
                            }
                        }
                        CommonAction::Delete => Ok(KeyHandleResultInternal::SkipRender),
                    }
                } else {
                    Ok(KeyHandleResultInternal::KeyNotHandled)
                }
            }
        }
    }
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum SearchActions {}

impl Default for SearchScreen {
    fn default() -> Self {
        Self {
            preview: None,
            phase: Phase::Search,
            songs_dir: Dir::default(),
            inputs: InputGroups::new(
                [
                    Textbox {
                        variant: TextboxVariant::AnyTag,
                        filter_key: Tag::Any,
                        label: " Any Tag         :",
                        value: String::new(),
                    },
                    Textbox {
                        variant: TextboxVariant::Artist,
                        filter_key: Tag::Artist,
                        label: " Artist          :",
                        value: String::new(),
                    },
                    Textbox {
                        variant: TextboxVariant::Album,
                        filter_key: Tag::Album,
                        label: " Album           :",
                        value: String::new(),
                    },
                    Textbox {
                        variant: TextboxVariant::AlbumArtist,
                        filter_key: Tag::AlbumArtist,
                        label: " Album Artist    :",
                        value: String::new(),
                    },
                    Textbox {
                        variant: TextboxVariant::Title,
                        filter_key: Tag::Title,
                        label: " Title           :",
                        value: String::new(),
                    },
                    Textbox {
                        variant: TextboxVariant::FileName,
                        filter_key: Tag::File,
                        label: " File Name       :",
                        value: String::new(),
                    },
                    Textbox {
                        variant: TextboxVariant::Genre,
                        filter_key: Tag::Genre,
                        label: " Genre           :",
                        value: String::new(),
                    },
                ],
                [
                    FilterInput {
                        label: " Search mode     :",
                        variant: FilterInputVariant::SelectFilterKind {
                            value: FilterKind::Contains,
                        },
                    },
                    FilterInput {
                        label: " Case Sensistive :",
                        variant: FilterInputVariant::SelectFilterCaseSensitive { value: false },
                    },
                ],
                [ButtonInput {
                    label: " Reset",
                    variant: ButtonInputVariant::Reset,
                }],
            ),
        }
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
struct InputGroups<const N1: usize, const N2: usize, const N3: usize> {
    textbox_inputs: [Textbox; N1],
    filter_inputs: [FilterInput; N2],
    button_inputs: [ButtonInput; N3],
    focused_idx: FocusedInput,
}

impl<const N1: usize, const N2: usize, const N3: usize> InputGroups<N1, N2, N3> {
    pub fn new(
        textbox_inputs: [Textbox; N1],
        filter_inputs: [FilterInput; N2],
        button_inputs: [ButtonInput; N3],
    ) -> Self {
        Self {
            textbox_inputs,
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
    List { filter_input_on: bool },
}

#[derive(Debug)]
struct Textbox {
    variant: TextboxVariant,
    value: String,
    label: &'static str,
    filter_key: Tag,
}

#[derive(Debug, PartialEq, Eq)]
enum TextboxVariant {
    AnyTag,
    Artist,
    Album,
    AlbumArtist,
    Title,
    FileName,
    Genre,
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
