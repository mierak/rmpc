use anyhow::Result;
use crossterm::event::KeyCode;
use enum_map::{Enum, EnumMap, enum_map};
use itertools::Itertools;
use log::error;
use ratatui::{
    Frame,
    layout::Flex,
    prelude::{Constraint, Layout, Rect},
    style::{Style, Styled},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
};

use super::{CommonAction, Pane};
use crate::{
    MpdQueryResult,
    config::{
        keys::{GlobalAction, QueueActions},
        tabs::PaneType,
        theme::properties::{Property, SongProperty},
    },
    context::AppContext,
    core::command::{create_env, run_external},
    mpd::{QueuePosition, commands::Song, mpd_client::MpdClient},
    shared::{
        ext::{btreeset_ranges::BTreeSetRanges, rect::RectExt},
        key_event::KeyEvent,
        macros::{modal, status_error, status_info, status_warn},
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::{
        UiEvent,
        dirstack::DirState,
        modals::{
            confirm_modal::ConfirmModal,
            info_list_modal::InfoListModal,
            input_modal::InputModal,
            select_modal::SelectModal,
        },
    },
};

#[derive(Debug)]
pub struct QueuePane {
    scrolling_state: DirState<TableState>,
    filter: Option<String>,
    filter_input_mode: bool,
    header: Vec<String>,
    column_widths: Vec<Constraint>,
    column_formats: Vec<Property<SongProperty>>,
    areas: EnumMap<Areas, Rect>,
    should_center_cursor_on_current: bool,
}

#[derive(Debug, Enum)]
enum Areas {
    Table,
    TableBlock,
    TableHeader,
    Scrollbar,
    FilterArea,
}

const ADD_TO_PLAYLIST: &str = "add_to_playlist";

impl QueuePane {
    pub fn new(context: &AppContext) -> Self {
        let (header, column_widths, column_formats) = Self::init(context);

        Self {
            scrolling_state: DirState::default(),
            filter: None,
            filter_input_mode: false,
            header,
            column_widths,
            column_formats,
            areas: enum_map! {
                _ => Rect::default(),
            },
            should_center_cursor_on_current: context.config.center_current_song_on_change,
        }
    }

    fn init(context: &AppContext) -> (Vec<String>, Vec<Constraint>, Vec<Property<SongProperty>>) {
        (
            context.config.theme.song_table_format.iter().map(|v| v.label.clone()).collect_vec(),
            context
                .config
                .theme
                .song_table_format
                .iter()
                // This 0 is fine - song_table_format should never have the Ratio constaint
                .map(|v| v.width.into_constraint(0))
                .collect_vec(),
            context.config.theme.song_table_format.iter().map(|v| v.prop.clone()).collect_vec(),
        )
    }

    fn filter_text(&self) -> Option<String> {
        self.filter
            .as_ref()
            .map(|v| format!("[FILTER]: {v}{} ", if self.filter_input_mode { "█" } else { "" }))
    }
}

impl Pane for QueuePane {
    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        context: &AppContext,
    ) -> anyhow::Result<()> {
        let AppContext { queue, config, .. } = context;
        let queue_len = queue.len();
        self.calculate_areas(area, context)?;

        let filter_text = self.filter_text();

        let table_block = {
            let border_style = config.as_border_style();
            let mut b = Block::default().border_style(border_style);
            if config.theme.show_song_table_header {
                b = b.borders(Borders::TOP);
            }
            if self.areas[Areas::FilterArea].height == 0 {
                if let Some(ref title) = filter_text {
                    b = b.title(title.clone().set_style(border_style));
                }
            }
            b
        };

        self.scrolling_state.set_content_len(Some(queue_len));

        let widths = Layout::horizontal(self.column_widths.as_slice())
            .flex(Flex::Start)
            .spacing(1)
            .split(self.areas[Areas::Table]);

        let formats = &config.theme.song_table_format;

        let offset = self.scrolling_state.as_render_state_ref().offset();
        let viewport_len = self.scrolling_state.viewport_len().unwrap_or_default();

        let marker_symbol_len = config.theme.symbols.marker.chars().count();
        let table_items = queue
            .iter()
            .enumerate()
            .map(|(idx, song)| {
                // Supply default row to skip unnecessary work for rows that are either below or
                // above the visible portion of the table
                if idx < offset || idx > viewport_len + offset {
                    return Row::new((0..formats.len()).map(|_| Cell::default()));
                }

                let is_current = context
                    .find_current_song_in_queue()
                    .map(|(_, song)| song.id)
                    .is_some_and(|v| v == song.id);

                let is_marked = self.scrolling_state.get_marked().contains(&idx);
                let columns = (0..formats.len()).map(|i| {
                    let mut max_len: usize = widths[i].width.into();
                    // We have to subtract marker symbol length from max len in
                    // order to make space for the marker
                    // symbol in case we are in the first column of the table
                    // and the song is marked.
                    if is_marked && i == 0 {
                        max_len = max_len.saturating_sub(marker_symbol_len);
                    }

                    let mut line = song
                        .as_line_ellipsized(
                            &formats[i].prop,
                            max_len,
                            &config.theme.symbols,
                            &config.theme.format_tag_separator,
                            config.theme.multiple_tag_resolution_strategy,
                        )
                        .unwrap_or_default()
                        .alignment(formats[i].alignment.into());

                    if is_marked && i == 0 {
                        let marker_span = Span::styled(
                            &config.theme.symbols.marker,
                            config.theme.highlighted_item_style,
                        );
                        line.spans.splice(..0, std::iter::once(marker_span));
                    }

                    line
                });

                let is_highlighted = is_current
                    || self
                        .filter
                        .as_ref()
                        .is_some_and(|filter| song.matches(self.column_formats.as_slice(), filter));

                if is_highlighted {
                    Row::new(
                        columns
                            .map(|column| column.patch_style(config.theme.highlighted_item_style)),
                    )
                    .style(config.theme.highlighted_item_style)
                } else {
                    Row::new(columns)
                }
            })
            .collect_vec();

        if config.theme.show_song_table_header {
            let header_table = Table::default()
                .header(Row::new(self.header.iter().enumerate().map(|(idx, title)| {
                    Line::from(title.as_str()).alignment(formats[idx].alignment.into())
                })))
                .style(config.as_text_style())
                .widths(self.column_widths.clone())
                .block(config.as_header_table_block());

            frame.render_widget(header_table, self.areas[Areas::TableHeader]);
        }

        let table = Table::new(table_items, self.column_widths.clone())
            .style(config.as_text_style())
            .row_highlight_style(config.theme.current_item_style);

        frame.render_widget(table_block, self.areas[Areas::TableBlock]);
        frame.render_stateful_widget(
            table,
            self.areas[Areas::Table],
            self.scrolling_state.as_render_state_ref(),
        );

        self.scrolling_state.set_viewport_len(Some(self.areas[Areas::Table].height.into()));
        if let Some(scrollbar) = config.as_styled_scrollbar() {
            frame.render_stateful_widget(
                scrollbar,
                self.areas[Areas::Scrollbar],
                self.scrolling_state.as_scrollbar_state_ref(),
            );
        }

        if let Some(filter_text) = filter_text {
            if self.areas[Areas::FilterArea].height > 0 {
                frame.render_widget(
                    Text::from(filter_text).style(
                        config.theme.text_color.map(|c| Style::default().fg(c)).unwrap_or_default(),
                    ),
                    self.areas[Areas::FilterArea],
                );
            }
        }

        Ok(())
    }

    fn calculate_areas(&mut self, area: Rect, context: &AppContext) -> Result<()> {
        let AppContext { config, .. } = context;

        let header_height: u16 = config.theme.show_song_table_header.into();
        let scrollbar_area_width: u16 = config.theme.scrollbar.is_some().into();

        let [header_area, queue_area] =
            Layout::vertical([Constraint::Length(header_height), Constraint::Min(0)]).areas(area);
        let [header_area, _scrollbar_placeholder] = Layout::horizontal([
            Constraint::Percentage(100),
            Constraint::Length(scrollbar_area_width),
        ])
        .areas(header_area);
        let [table_block_area, scrollbar_area] = Layout::horizontal([
            Constraint::Percentage(100),
            Constraint::Length(scrollbar_area_width),
        ])
        .areas(queue_area);

        // Apply empty margin on left and right
        let table_block_area = table_block_area.shrink_horizontally(1);
        let header_area = header_area.shrink_horizontally(1);
        // Make scrollbar not overlap header/table separator if separator is visible
        let scrollbar_area =
            scrollbar_area.shrink_from_top(config.theme.show_song_table_header.into());

        let table_area = if config.theme.show_song_table_header {
            table_block_area.shrink_from_top(1)
        } else {
            table_block_area
        };

        let table_area = if self.filter.is_some() && !context.config.theme.show_song_table_header {
            self.areas[Areas::FilterArea] =
                Rect::new(table_area.x, table_area.y, table_area.width, 1);
            table_area.shrink_from_top(1)
        } else {
            self.areas[Areas::FilterArea] = Rect::default();
            table_area
        };

        self.areas[Areas::Table] = table_area;
        self.areas[Areas::TableBlock] = table_block_area;
        self.areas[Areas::TableHeader] = header_area;
        self.areas[Areas::Scrollbar] = scrollbar_area;

        Ok(())
    }

    fn before_show(&mut self, context: &AppContext) -> Result<()> {
        self.scrolling_state.set_content_len(Some(context.queue.len()));
        self.scrolling_state.set_viewport_len(Some(self.areas[Areas::Table].height as usize));

        if self.should_center_cursor_on_current {
            let to_select = context
                .find_current_song_in_queue()
                .map(|(idx, _)| idx)
                .or(self.scrolling_state.get_selected())
                .or(Some(0));
            self.scrolling_state.select(to_select, usize::MAX);
            self.should_center_cursor_on_current = false;
        } else {
            let to_select = self
                .scrolling_state
                .get_selected()
                .or(context.find_current_song_in_queue().map(|v| v.0).or(Some(0)));
            self.scrolling_state.select(to_select, context.config.scrolloff);
        }

        Ok(())
    }

    fn resize(&mut self, _area: Rect, context: &AppContext) -> Result<()> {
        self.scrolling_state.set_viewport_len(Some(self.areas[Areas::Table].height as usize));
        let to_select = self
            .scrolling_state
            .get_selected()
            .or(context.find_current_song_in_queue().map(|v| v.0).or(Some(0)));
        self.scrolling_state.select(to_select, context.config.scrolloff);
        context.render()?;
        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut UiEvent,
        is_visible: bool,
        context: &AppContext,
    ) -> Result<()> {
        match event {
            UiEvent::SongChanged => {
                if let Some((idx, _)) = context.find_current_song_in_queue() {
                    if context.config.select_current_song_on_change {
                        match (is_visible, context.config.center_current_song_on_change) {
                            (true, true) => {
                                self.scrolling_state.select(Some(idx), usize::MAX);
                            }
                            (false, true) => {
                                self.scrolling_state.select(Some(idx), usize::MAX);
                                self.should_center_cursor_on_current = true;
                            }
                            (true, false) | (false, false) => {
                                self.scrolling_state.select(Some(idx), context.config.scrolloff);
                            }
                        }

                        context.render()?;
                    }
                }
            }
            UiEvent::Reconnected => {
                self.before_show(context)?;
            }
            UiEvent::ConfigChanged => {
                let (header, column_widths, column_formats) = Self::init(context);
                self.header = header;
                self.column_formats = column_formats;
                self.column_widths = column_widths;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &AppContext) -> Result<()> {
        if !self.areas[Areas::Table].contains(event.into()) {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::LeftClick => {
                let clicked_row: usize = event.y.saturating_sub(self.areas[Areas::Table].y).into();
                if let Some(idx) = self.scrolling_state.get_at_rendered_row(clicked_row) {
                    self.scrolling_state.select(Some(idx), context.config.scrolloff);

                    context.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                let clicked_row: usize = event.y.saturating_sub(self.areas[Areas::Table].y).into();

                if let Some(song) = self
                    .scrolling_state
                    .get_at_rendered_row(clicked_row)
                    .and_then(|idx| context.queue.get(idx))
                {
                    let id = song.id;
                    context.command(move |client| {
                        client.play_id(id)?;
                        Ok(())
                    });
                }
            }
            MouseEventKind::MiddleClick => {
                let clicked_row: usize = event.y.saturating_sub(self.areas[Areas::Table].y).into();

                if let Some(selected_song) = self
                    .scrolling_state
                    .get_at_rendered_row(clicked_row)
                    .and_then(|idx| context.queue.get(idx))
                {
                    let id = selected_song.id;
                    context.command(move |client| {
                        client.delete_id(id)?;
                        Ok(())
                    });
                }
            }
            MouseEventKind::ScrollDown => {
                self.scrolling_state.next(context.config.scrolloff, false);
                context.render()?;
            }
            MouseEventKind::ScrollUp => {
                self.scrolling_state.prev(context.config.scrolloff, false);
                context.render()?;
            }
            MouseEventKind::RightClick => {}
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
            (ADD_TO_PLAYLIST, MpdQueryResult::AddToPlaylist { playlists, song_file }) => {
                modal!(
                    context,
                    SelectModal::builder()
                        .context(context)
                        .options(playlists)
                        .confirm_label("Add")
                        .title("Select a playlist")
                        .on_confirm(move |context, selected, _idx| {
                            let song_file = song_file.clone();
                            context.command(move |client| {
                                if song_file.starts_with('/') {
                                    client.add_to_playlist(
                                        &selected,
                                        &format!("file://{song_file}"),
                                        None,
                                    )?;
                                } else {
                                    client.add_to_playlist(&selected, &song_file, None)?;
                                }
                                status_info!("Song added to playlist {}", selected);
                                Ok(())
                            });
                            Ok(())
                        })
                        .build()
                );
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_action(&mut self, event: &mut KeyEvent, context: &mut AppContext) -> Result<()> {
        if self.filter_input_mode {
            match event.as_common_action(context) {
                Some(CommonAction::Confirm) => {
                    self.filter_input_mode = false;

                    context.render()?;
                }
                Some(CommonAction::Close) => {
                    self.filter_input_mode = false;
                    self.filter = None;

                    context.render()?;
                }
                _ => {
                    event.stop_propagation();
                    match event.code() {
                        KeyCode::Char(c) => {
                            if let Some(ref mut f) = self.filter {
                                f.push(c);
                            }
                            self.jump_first(&context.queue, context.config.scrolloff);

                            context.render()?;
                        }
                        KeyCode::Backspace => {
                            if let Some(ref mut f) = self.filter {
                                f.pop();
                            }

                            context.render()?;
                        }
                        _ => {}
                    }
                }
            }
        } else if let Some(action) = event.as_queue_action(context) {
            match action {
                QueueActions::Delete if !self.scrolling_state.marked.is_empty() => {
                    for range in self.scrolling_state.marked.ranges().rev() {
                        context.command(move |client| {
                            client.delete_from_queue(range.into())?;
                            Ok(())
                        });
                    }
                    self.scrolling_state.marked.clear();
                    status_info!("Marked songs removed from queue");
                    context.render()?;
                }
                QueueActions::Delete => {
                    if let Some(selected_song) =
                        self.scrolling_state.get_selected().and_then(|idx| context.queue.get(idx))
                    {
                        let id = selected_song.id;
                        context.command(move |client| {
                            client.delete_id(id)?;
                            Ok(())
                        });
                    } else {
                        status_error!("No song selected");
                    }
                }
                QueueActions::DeleteAll => {
                    modal!(
                        context,
                        ConfirmModal::builder().context(context)
                            .message("Are you sure you want to clear the queue? This action cannot be undone.")
                            .on_confirm(|context| {
                                context.command(|client| Ok(client.clear()?));
                                Ok(())
                            })
                            .confirm_label("Clear")
                            .size((45, 6))
                            .build()
                    );
                }
                QueueActions::Play => {
                    if let Some(selected_song) =
                        self.scrolling_state.get_selected().and_then(|idx| context.queue.get(idx))
                    {
                        let id = selected_song.id;
                        context.command(move |client| {
                            client.play_id(id)?;
                            Ok(())
                        });
                    }
                }
                QueueActions::JumpToCurrent => {
                    if let Some((idx, _)) = context.find_current_song_in_queue() {
                        self.scrolling_state.select(Some(idx), context.config.scrolloff);
                        context.render()?;
                    } else {
                        status_info!("No song is currently playing");
                    }
                }
                QueueActions::Save => {
                    modal!(
                        context,
                        InputModal::new(context)
                            .title("Save queue as playlist")
                            .confirm_label("Save")
                            .input_label("Playlist name:")
                            .on_confirm(move |context, value| {
                                let value = value.to_owned();
                                context.command(move |client| {
                                    match client.save_queue_as_playlist(&value, None) {
                                        Ok(()) => {
                                            status_info!("Playlist '{}' saved", value);
                                        }
                                        Err(err) => {
                                            status_error!(err:?; "Failed to save playlist '{}'",value);
                                        }
                                    }
                                    Ok(())
                                });
                                Ok(())
                            })
                    );
                }
                QueueActions::AddToPlaylist => {
                    if let Some(selected_song) =
                        self.scrolling_state.get_selected().and_then(|idx| context.queue.get(idx))
                    {
                        let uri = selected_song.file.clone();
                        context
                            .query()
                            .id(ADD_TO_PLAYLIST)
                            .replace_id(ADD_TO_PLAYLIST)
                            .target(PaneType::Queue)
                            .query(move |client| {
                                let playlists = client
                                    .list_playlists()?
                                    .into_iter()
                                    .map(|v| v.name)
                                    .sorted()
                                    .collect_vec();
                                Ok(MpdQueryResult::AddToPlaylist { playlists, song_file: uri })
                            });
                    }
                }
                QueueActions::Shuffle if !self.scrolling_state.marked.is_empty() => {
                    for range in self.scrolling_state.marked.ranges().rev() {
                        context.command(move |client| {
                            client.shuffle(Some(range.into()))?;
                            Ok(())
                        });
                    }
                    status_info!("Shuffled selected songs");
                }
                QueueActions::Shuffle => {
                    context.command(move |client| {
                        client.shuffle(None)?;
                        Ok(())
                    });
                    status_info!("Shuffled the queue");
                }
                QueueActions::Unused => {}
            }
        } else if let Some(action) = event.as_common_action(context) {
            match action {
                CommonAction::Up => {
                    if !context.queue.is_empty() {
                        self.scrolling_state
                            .prev(context.config.scrolloff, context.config.wrap_navigation);
                    }

                    context.render()?;
                }
                CommonAction::Down => {
                    if !context.queue.is_empty() {
                        self.scrolling_state
                            .next(context.config.scrolloff, context.config.wrap_navigation);
                    }

                    context.render()?;
                }
                CommonAction::MoveUp if !self.scrolling_state.get_marked().is_empty() => {
                    if context.queue.is_empty() {
                        return Ok(());
                    }

                    if let Some(0) = self.scrolling_state.marked.first() {
                        return Ok(());
                    }

                    for range in self.scrolling_state.marked.ranges() {
                        for idx in range.clone() {
                            let new_idx = idx.saturating_sub(1);
                            context.queue.swap(idx, new_idx);
                        }

                        let new_start_idx = range.start().saturating_sub(1);
                        context.command(move |client| {
                            client.move_in_queue(
                                range.into(),
                                QueuePosition::Absolute(new_start_idx),
                            )?;
                            Ok(())
                        });
                    }

                    if let Some(start) = self.scrolling_state.marked.first() {
                        let new_idx = start.saturating_sub(1);
                        self.scrolling_state.select(Some(new_idx), context.config.scrolloff);
                    }

                    let mut new_marked =
                        self.scrolling_state.marked.iter().map(|i| i.saturating_sub(1)).collect();
                    std::mem::swap(&mut self.scrolling_state.marked, &mut new_marked);

                    context.render()?;
                    return Ok(());
                }
                CommonAction::MoveDown if !self.scrolling_state.get_marked().is_empty() => {
                    if context.queue.is_empty() {
                        return Ok(());
                    }

                    if let Some(last_idx) = self.scrolling_state.marked.last() {
                        if *last_idx == context.queue.len() - 1 {
                            return Ok(());
                        }
                    }

                    for range in self.scrolling_state.marked.ranges().rev() {
                        for idx in range.clone().rev() {
                            let new_idx = idx.saturating_add(1);
                            context.queue.swap(idx, new_idx);
                        }

                        let new_start_idx = range.start().saturating_add(1);
                        context.command(move |client| {
                            client.move_in_queue(
                                range.into(),
                                QueuePosition::Absolute(new_start_idx),
                            )?;
                            Ok(())
                        });
                    }

                    if let Some(start) = self.scrolling_state.marked.last() {
                        let new_idx = start.saturating_add(1);
                        self.scrolling_state.select(Some(new_idx), context.config.scrolloff);
                    }

                    let mut new_marked =
                        self.scrolling_state.marked.iter().map(|i| i.saturating_add(1)).collect();
                    std::mem::swap(&mut self.scrolling_state.marked, &mut new_marked);

                    context.render()?;
                    return Ok(());
                }
                CommonAction::MoveUp => {
                    if context.queue.is_empty() {
                        return Ok(());
                    }

                    let Some(idx) = self.scrolling_state.get_selected() else {
                        return Ok(());
                    };

                    let Some(selected) =
                        self.scrolling_state.get_selected().and_then(|idx| context.queue.get(idx))
                    else {
                        return Ok(());
                    };

                    let new_idx = idx.saturating_sub(1);
                    let id = selected.id;
                    context.command(move |client| {
                        client.move_id(id, QueuePosition::Absolute(new_idx))?;
                        Ok(())
                    });
                    self.scrolling_state.select(Some(new_idx), context.config.scrolloff);
                    context.queue.swap(idx, new_idx);
                    context.render()?;
                }
                CommonAction::MoveDown => {
                    if context.queue.is_empty() {
                        return Ok(());
                    }

                    let Some(idx) = self.scrolling_state.get_selected() else {
                        return Ok(());
                    };
                    let Some(selected) =
                        self.scrolling_state.get_selected().and_then(|idx| context.queue.get(idx))
                    else {
                        return Ok(());
                    };

                    let new_idx = (idx + 1).min(context.queue.len() - 1);
                    let id = selected.id;
                    context.command(move |client| {
                        client.move_id(id, QueuePosition::Absolute(new_idx))?;
                        Ok(())
                    });
                    self.scrolling_state.select(Some(new_idx), context.config.scrolloff);
                    context.queue.swap(idx, new_idx);
                    context.render()?;
                }
                CommonAction::DownHalf => {
                    if !context.queue.is_empty() {
                        self.scrolling_state.next_half_viewport(context.config.scrolloff);
                    }

                    context.render()?;
                }
                CommonAction::UpHalf => {
                    if !context.queue.is_empty() {
                        self.scrolling_state.prev_half_viewport(context.config.scrolloff);
                    }

                    context.render()?;
                }
                CommonAction::PageDown => {
                    if !context.queue.is_empty() {
                        self.scrolling_state.next_viewport(context.config.scrolloff);
                    }

                    context.render()?;
                }
                CommonAction::PageUp => {
                    if !context.queue.is_empty() {
                        self.scrolling_state.prev_viewport(context.config.scrolloff);
                    }

                    context.render()?;
                }
                CommonAction::Bottom => {
                    if !context.queue.is_empty() {
                        self.scrolling_state.last();
                    }

                    context.render()?;
                }
                CommonAction::Top => {
                    if !context.queue.is_empty() {
                        self.scrolling_state.first();
                    }

                    context.render()?;
                }
                CommonAction::Right => {}
                CommonAction::Left => {}
                CommonAction::EnterSearch => {
                    self.filter_input_mode = true;
                    self.filter = Some(String::new());

                    context.render()?;
                }
                CommonAction::NextResult => {
                    self.jump_forward(&context.queue, context.config.scrolloff);

                    context.render()?;
                }
                CommonAction::PreviousResult => {
                    self.jump_back(&context.queue, context.config.scrolloff);

                    context.render()?;
                }
                CommonAction::Select => {
                    if let Some(sel) = self.scrolling_state.get_selected() {
                        self.scrolling_state.toggle_mark(sel);
                        self.scrolling_state
                            .next(context.config.scrolloff, context.config.wrap_navigation);

                        context.render()?;
                    }
                }
                CommonAction::InvertSelection => {
                    self.scrolling_state.invert_marked();

                    context.render()?;
                }
                CommonAction::Add => {}
                CommonAction::AddAll => {}
                CommonAction::Insert => {
                    let song_under_cursor =
                        self.scrolling_state.get_selected().and_then(|idx| context.queue.get(idx));

                    if let Some(song) = song_under_cursor {
                        let file = song.file.clone();
                        context.command(move |client| {
                            client.add(&file, Some(QueuePosition::RelativeAdd(0)))?;
                            Ok(())
                        });
                    }
                }
                CommonAction::ShowInfo => {
                    if let Some(selected_song) =
                        self.scrolling_state.get_selected().and_then(|idx| context.queue.get(idx))
                    {
                        modal!(
                            context,
                            InfoListModal::builder()
                                .items(selected_song)
                                .title("Song info")
                                .column_widths(&[30, 70])
                                .build()
                        );
                    } else {
                        status_error!("No song selected");
                    }
                }
                CommonAction::InsertAll => {}
                CommonAction::AddReplace => {}
                CommonAction::AddAllReplace => {}
                CommonAction::Delete => {}
                CommonAction::Rename => {}
                CommonAction::Close => {}
                CommonAction::FocusInput => {}
                CommonAction::Confirm => {} // queue has its own binding for
                // play
                CommonAction::PaneDown => {}
                CommonAction::PaneUp => {}
                CommonAction::PaneRight => {}
                CommonAction::PaneLeft => {}
            }
        } else if let Some(action) = event.as_global_action(context) {
            match action {
                GlobalAction::ExternalCommand { command, .. } => {
                    let song = self
                        .scrolling_state
                        .get_selected()
                        .and_then(|idx| context.queue.get(idx).map(|song| song.file.as_str()));

                    run_external(command.clone(), create_env(context, song));
                }
                _ => {
                    event.abandon();
                }
            }
        }

        Ok(())
    }
}

impl QueuePane {
    pub fn jump_forward(&mut self, queue: &[Song], scrolloff: usize) {
        let Some(filter) = self.filter.as_ref() else {
            status_warn!("No filter set");
            return;
        };
        let Some(selected) = self.scrolling_state.get_selected() else {
            error!(state:? = self.scrolling_state; "No song selected");
            return;
        };

        let length = queue.len();
        for i in selected + 1..length + selected {
            let i = i % length;
            if queue[i].matches(self.column_formats.as_slice(), filter) {
                self.scrolling_state.select(Some(i), scrolloff);
                break;
            }
        }
    }

    pub fn jump_back(&mut self, queue: &[Song], scrolloff: usize) {
        let Some(filter) = self.filter.as_ref() else {
            status_warn!("No filter set");
            return;
        };
        let Some(selected) = self.scrolling_state.get_selected() else {
            error!(state:? = self.scrolling_state; "No song selected");
            return;
        };

        let length = queue.len();
        for i in (0..length).rev() {
            let i = (i + selected) % length;
            if queue[i].matches(self.column_formats.as_slice(), filter) {
                self.scrolling_state.select(Some(i), scrolloff);
                break;
            }
        }
    }

    pub fn jump_first(&mut self, queue: &[Song], scrolloff: usize) {
        let Some(filter) = self.filter.as_ref() else {
            status_warn!("No filter set");
            return;
        };

        queue
            .iter()
            .enumerate()
            .find(|(_, item)| item.matches(self.column_formats.as_slice(), filter))
            .inspect(|(idx, _)| self.scrolling_state.select(Some(*idx), scrolloff));
    }
}
