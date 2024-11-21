use anyhow::Result;
use crossterm::event::KeyCode;
use itertools::Itertools;

use crate::{
    cli::{create_env, run_external},
    config::{
        keys::{GlobalAction, QueueActions},
        theme::{
            properties::{Property, SongProperty},
            PercentOrLength,
        },
    },
    context::AppContext,
    mpd::{
        commands::Song,
        mpd_client::{MpdClient, QueueMoveTarget},
    },
    shared::{
        ext::btreeset_ranges::BTreeSetRanges,
        key_event::KeyEvent,
        macros::{modal, status_error, status_info, status_warn},
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::{
        dirstack::DirState,
        modals::{
            confirm_modal::ConfirmModal, input_modal::InputModal, select_modal::SelectModal, song_info::SongInfoModal,
        },
        UiEvent,
    },
};
use log::error;
use ratatui::{
    layout::Flex,
    prelude::{Constraint, Layout, Rect},
    style::Stylize,
    text::{Line, Span},
    widgets::{Block, Borders, Row, Table, TableState},
    Frame,
};

use super::{CommonAction, Pane};

#[derive(Debug)]
pub struct QueuePane {
    scrolling_state: DirState<TableState>,
    filter: Option<String>,
    filter_input_mode: bool,
    header: Vec<&'static str>,
    column_widths: Vec<Constraint>,
    column_formats: Vec<&'static Property<'static, SongProperty>>,
    table_area: Rect,
}

impl QueuePane {
    pub fn new(context: &AppContext) -> Self {
        let config = context.config;
        Self {
            scrolling_state: DirState::default(),
            filter: None,
            filter_input_mode: false,
            header: config.theme.song_table_format.iter().map(|v| v.label).collect_vec(),
            column_widths: config
                .theme
                .song_table_format
                .iter()
                .map(|v| match v.width {
                    PercentOrLength::Percent(p) => Constraint::Percentage(p),
                    PercentOrLength::Length(l) => Constraint::Length(l),
                })
                .collect_vec(),
            column_formats: config.theme.song_table_format.iter().map(|v| v.prop).collect_vec(),
            table_area: Rect::default(),
        }
    }
}

impl Pane for QueuePane {
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> anyhow::Result<()> {
        let AppContext { queue, config, .. } = context;
        let queue_len = queue.len();

        let title = self
            .filter
            .as_ref()
            .map(|v| format!("[FILTER]: {v}{} ", if self.filter_input_mode { "█" } else { "" }));

        let table_block = {
            let mut b = Block::default().border_style(config.as_border_style().bold());
            if config.theme.show_song_table_header {
                b = b.borders(Borders::TOP);
            }
            if let Some(ref title) = title {
                b = b.title(title.clone().blue());
            }
            b
        };

        let header_height = u16::from(config.theme.show_song_table_header);

        let [data_area, scrollbar_area] =
            Layout::horizontal([Constraint::Percentage(100), Constraint::Length(1)]).areas(area);

        let [table_header_section, mut queue_section] =
            Layout::vertical([Constraint::Length(header_height), Constraint::Min(0)])
                .horizontal_margin(1)
                .areas(data_area);

        let constraints: &[Constraint] = if config.theme.show_song_table_header {
            &[Constraint::Length(header_height + 1), Constraint::Min(0)]
        } else {
            &[Constraint::Min(0)]
        };
        let scrollbar_index = usize::from(config.theme.show_song_table_header);
        let scrollbar_area = Layout::vertical(constraints).split(scrollbar_area)[scrollbar_index];

        let table_area = table_block.inner(queue_section);

        self.scrolling_state.set_content_len(Some(queue_len));

        let widths = Layout::horizontal(self.column_widths.clone())
            .flex(Flex::Start)
            .spacing(1)
            .split(table_area);

        let formats = &config.theme.song_table_format;

        let marker_symbol_len = config.theme.symbols.marker.chars().count();
        let table_items = queue
            .iter()
            .enumerate()
            .map(|(idx, song)| {
                let is_current = context
                    .find_current_song_in_queue()
                    .map(|(_, song)| song.id)
                    .is_some_and(|v| v == song.id);

                let is_marked = self.scrolling_state.get_marked().contains(&idx);
                let columns = (0..formats.len()).map(|i| {
                    let mut max_len: usize = widths[i].width.into();
                    // We have to subtract marker symbol length from max len in order to make space
                    // for the marker symbol in case we are in the first column of the table and the
                    // song is marked.
                    if is_marked && i == 0 {
                        max_len = max_len.saturating_sub(marker_symbol_len);
                    }

                    let mut line = song
                        .as_line_ellipsized(formats[i].prop, max_len, &config.theme.symbols)
                        .unwrap_or_default()
                        .alignment(formats[i].alignment.into());

                    if is_marked && i == 0 {
                        let marker_span =
                            Span::styled(config.theme.symbols.marker, config.theme.highlighted_item_style);
                        line.spans.splice(..0, std::iter::once(marker_span));
                    };

                    line
                });

                let is_highlighted = is_current
                    || self
                        .filter
                        .as_ref()
                        .is_some_and(|filter| song.matches(self.column_formats.as_slice(), filter));

                if is_highlighted {
                    Row::new(columns.map(|column| column.patch_style(config.theme.highlighted_item_style)))
                        .style(config.theme.highlighted_item_style)
                } else {
                    Row::new(columns)
                }
            })
            .collect_vec();

        if config.theme.show_song_table_header {
            let header_table = Table::default()
                .header(Row::new(self.header.iter().enumerate().map(|(idx, title)| {
                    Line::from(*title).alignment(formats[idx].alignment.into())
                })))
                .style(config.as_text_style())
                .widths(self.column_widths.clone())
                .block(config.as_header_table_block());
            frame.render_widget(header_table, table_header_section);
        }

        let table = Table::new(table_items, self.column_widths.clone())
            .style(config.as_text_style())
            .row_highlight_style(config.theme.current_item_style);

        self.table_area = table_area;
        frame.render_stateful_widget(table, table_area, self.scrolling_state.as_render_state_ref());
        frame.render_widget(table_block, queue_section);

        if config.theme.show_song_table_header {
            queue_section.y = queue_section.y.saturating_add(1);
            queue_section.height = queue_section.height.saturating_sub(1);
        }
        self.scrolling_state.set_viewport_len(Some(queue_section.height.into()));
        frame.render_stateful_widget(
            config.as_styled_scrollbar(),
            scrollbar_area,
            self.scrolling_state.as_scrollbar_state_ref(),
        );

        Ok(())
    }

    fn before_show(&mut self, _client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        self.scrolling_state.set_content_len(Some(context.queue.len()));
        let scrolloff = if self.table_area == Rect::default() {
            0
        } else {
            context.config.scrolloff
        };
        if self.scrolling_state.get_selected().is_none() {
            self.scrolling_state
                .select(context.find_current_song_in_queue().map(|v| v.0).or(Some(0)), scrolloff);
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, _client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        if let UiEvent::SongChanged = event {
            if let Some((idx, _)) = context.find_current_song_in_queue() {
                if context.config.select_current_song_on_change {
                    self.scrolling_state.select(Some(idx), context.config.scrolloff);
                    context.render()?;
                }
            }
        };

        Ok(())
    }

    fn handle_mouse_event(
        &mut self,
        event: MouseEvent,
        client: &mut impl MpdClient,
        context: &mut AppContext,
    ) -> Result<()> {
        if !self.table_area.contains(event.into()) {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::LeftClick => {
                let clicked_row: usize = event.y.saturating_sub(self.table_area.y).into();
                if let Some(idx) = self.scrolling_state.get_at_rendered_row(clicked_row) {
                    self.scrolling_state.select(Some(idx), context.config.scrolloff);

                    context.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                let clicked_row: usize = event.y.saturating_sub(self.table_area.y).into();

                if let Some(song) = self
                    .scrolling_state
                    .get_at_rendered_row(clicked_row)
                    .and_then(|idx| context.queue.get(idx))
                {
                    client.play_id(song.id)?;
                    context.render()?;
                }
            }
            MouseEventKind::MiddleClick => {
                let clicked_row: usize = event.y.saturating_sub(self.table_area.y).into();

                if let Some(selected_song) = self
                    .scrolling_state
                    .get_at_rendered_row(clicked_row)
                    .and_then(|idx| context.queue.get(idx))
                {
                    client.delete_id(selected_song.id)?;
                    context.render()?;
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

    fn handle_action(&mut self, event: &mut KeyEvent, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
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
                            };
                            self.jump_first(&context.queue, context.config.scrolloff);

                            context.render()?;
                        }
                        KeyCode::Backspace => {
                            if let Some(ref mut f) = self.filter {
                                f.pop();
                            };

                            context.render()?;
                        }
                        _ => {}
                    }
                }
            }
        } else if let Some(action) = event.as_queue_action(context) {
            match action {
                QueueActions::Delete => {
                    if let Some(selected_song) = self
                        .scrolling_state
                        .get_selected()
                        .and_then(|idx| context.queue.get(idx))
                    {
                        match client.delete_id(selected_song.id) {
                            Ok(()) => {}
                            Err(e) => error!("{:?}", e),
                        }
                    } else {
                        status_error!("No song selected");
                    }
                }
                QueueActions::DeleteAll => {
                    modal!(
                        context,
                        ConfirmModal::new(context)
                            .message("Are you sure you want to clear the queue? This action cannot be undone.")
                            .on_confirm(|client| Ok(client.clear()?))
                            .confirm_label("Clear")
                            .size(45, 6)
                    );
                }
                QueueActions::Play => {
                    if let Some(selected_song) = self
                        .scrolling_state
                        .get_selected()
                        .and_then(|idx| context.queue.get(idx))
                    {
                        client.play_id(selected_song.id)?;
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
                            .on_confirm(move |client, value| {
                                match client.save_queue_as_playlist(value, None) {
                                    Ok(()) => {
                                        status_info!("Playlist '{}' saved", value);
                                    }
                                    Err(err) => {
                                        status_error!(err:?; "Failed to save playlist '{}'",value);
                                    }
                                };
                                Ok(())
                            })
                    );
                }
                QueueActions::AddToPlaylist => {
                    if let Some(selected_song) = self
                        .scrolling_state
                        .get_selected()
                        .and_then(|idx| context.queue.get(idx))
                    {
                        let playlists = client
                            .list_playlists()?
                            .into_iter()
                            .map(|v| v.name)
                            .sorted()
                            .collect_vec();
                        let uri = selected_song.file.clone();
                        modal!(
                            context,
                            SelectModal::new(context)
                                .options(playlists)
                                .confirm_label("Add")
                                .title("Select a playlist")
                                .on_confirm(move |client, selected: &String, _idx| {
                                    client.add_to_playlist(selected, &uri, None)?;
                                    status_info!("Song added to playlist {}", selected);
                                    Ok(())
                                })
                        );
                    }
                }
                QueueActions::ShowInfo => {
                    if let Some(selected_song) = self
                        .scrolling_state
                        .get_selected()
                        .and_then(|idx| context.queue.get(idx))
                    {
                        modal!(context, SongInfoModal::new(selected_song.clone()));
                    } else {
                        status_error!("No song selected");
                    }
                }
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

                    let marked = &self.scrolling_state.marked;
                    if let Some(0) = marked.first() {
                        return Ok(());
                    }

                    for range in marked.ranges() {
                        let new_idx = range.start().saturating_sub(1);
                        client.move_in_queue(range.into(), QueueMoveTarget::Absolute(new_idx))?;
                    }

                    let mut new_marked = marked.iter().map(|i| i.saturating_sub(1)).collect();
                    std::mem::swap(&mut self.scrolling_state.marked, &mut new_marked);

                    return Ok(());
                }
                CommonAction::MoveDown if !self.scrolling_state.get_marked().is_empty() => {
                    if context.queue.is_empty() {
                        return Ok(());
                    }

                    let marked = &self.scrolling_state.marked;
                    if let Some(last_idx) = marked.last() {
                        if *last_idx == context.queue.len() - 1 {
                            return Ok(());
                        }
                    }

                    for range in marked.ranges().rev() {
                        let new_idx = range.start().saturating_add(1);
                        client.move_in_queue(range.into(), QueueMoveTarget::Absolute(new_idx))?;
                    }

                    let mut new_marked = marked.iter().map(|i| i.saturating_add(1)).collect();
                    std::mem::swap(&mut self.scrolling_state.marked, &mut new_marked);

                    return Ok(());
                }
                CommonAction::MoveUp => {
                    if context.queue.is_empty() {
                        return Ok(());
                    }

                    let Some(idx) = self.scrolling_state.get_selected() else {
                        return Ok(());
                    };

                    let Some(selected) = self
                        .scrolling_state
                        .get_selected()
                        .and_then(|idx| context.queue.get(idx))
                    else {
                        return Ok(());
                    };

                    let new_idx = idx.saturating_sub(1);
                    client.move_id(selected.id, QueueMoveTarget::Absolute(new_idx))?;
                    self.scrolling_state.select(Some(new_idx), context.config.scrolloff);
                }
                CommonAction::MoveDown => {
                    if context.queue.is_empty() {
                        return Ok(());
                    }

                    let Some(idx) = self.scrolling_state.get_selected() else {
                        return Ok(());
                    };
                    let Some(selected) = self
                        .scrolling_state
                        .get_selected()
                        .and_then(|idx| context.queue.get(idx))
                    else {
                        return Ok(());
                    };

                    let new_idx = (idx + 1).min(context.queue.len() - 1);
                    client.move_id(selected.id, QueueMoveTarget::Absolute(new_idx))?;
                    self.scrolling_state.select(Some(new_idx), context.config.scrolloff);
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
                    };
                }
                CommonAction::InvertSelection => {
                    self.scrolling_state.invert_marked();

                    context.render()?;
                }
                CommonAction::Add => {}
                CommonAction::AddAll => {}
                CommonAction::Delete => {}
                CommonAction::Rename => {}
                CommonAction::Close => {}
                CommonAction::FocusInput => {}
                CommonAction::Confirm => {} // queue has its own binding for play
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

                    run_external(command, create_env(context, song, client)?);
                }
                _ => {
                    event.abandon();
                }
            }
        };

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
