use anyhow::Result;
use crossterm::event::KeyCode;
use enum_map::{Enum, EnumMap, enum_map};
use itertools::Itertools;
use ratatui::{
    Frame,
    layout::Flex,
    prelude::{Constraint, Layout, Rect},
    style::{Style, Styled, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
};

use super::{CommonAction, Pane};
use crate::{
    MpdQueryResult,
    config::{
        keys::{
            GlobalAction,
            QueueActions,
            actions::{AddKind, AutoplayKind, RateKind, SaveKind},
        },
        tabs::PaneType,
        theme::{
            AlbumSeparator,
            properties::{Property, SongProperty},
        },
    },
    core::command::{create_env, run_external},
    ctx::{Ctx, LIKE_STICKER, RATING_STICKER},
    mpd::{
        QueuePosition,
        commands::Song,
        mpd_client::{MpdClient, SingleOrRange},
    },
    shared::{
        ext::{btreeset_ranges::BTreeSetRanges, rect::RectExt},
        key_event::KeyEvent,
        macros::{modal, status_error, status_info, status_warn},
        mouse_event::{MouseEvent, MouseEventKind, calculate_scrollbar_position},
        mpd_client_ext::{Autoplay, Enqueue, MpdClientExt},
    },
    ui::{
        UiEvent,
        dirstack::Dir,
        modals::{
            confirm_modal::ConfirmModal,
            info_list_modal::InfoListModal,
            input_modal::InputModal,
            menu::{create_add_modal, create_rating_modal, create_save_modal, modal::MenuModal},
            select_modal::SelectModal,
        },
    },
};

#[derive(Debug)]
pub struct QueuePane {
    queue: Dir<Song, TableState>,
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
const ADD_TO_PLAYLIST_MULTIPLE: &str = "add_to_playlist_multiple";

impl QueuePane {
    pub fn new(ctx: &Ctx) -> Self {
        let (header, column_widths, column_formats) = Self::init(ctx);

        Self {
            queue: Dir::new(ctx.queue.clone()),
            filter_input_mode: false,
            header,
            column_widths,
            column_formats,
            areas: enum_map! {
                _ => Rect::default(),
            },
            should_center_cursor_on_current: ctx.config.center_current_song_on_change,
        }
    }

    fn init(ctx: &Ctx) -> (Vec<String>, Vec<Constraint>, Vec<Property<SongProperty>>) {
        (
            ctx.config.theme.song_table_format.iter().map(|v| v.label.clone()).collect_vec(),
            ctx.config
                .theme
                .song_table_format
                .iter()
                // This 0 is fine - song_table_format should never have the Ratio constraint
                .map(|v| v.width.into_constraint(0))
                .collect_vec(),
            ctx.config.theme.song_table_format.iter().map(|v| v.prop.clone()).collect_vec(),
        )
    }

    fn enqueue_items(&self, all: bool) -> (Vec<Enqueue>, Option<usize>) {
        let hovered = self.queue.selected().map(|s| s.file.as_str());
        self.items(all).fold((Vec::new(), None), |mut acc, (idx, song)| {
            let path = song.file.clone();
            if hovered.as_ref().is_some_and(|hovered| hovered == &path) {
                acc.1 = Some(idx);
            }

            acc.0.push(Enqueue::File { path });

            acc
        })
    }

    fn items<'a>(&'a self, all: bool) -> Box<dyn Iterator<Item = (usize, &'a Song)> + 'a> {
        if all {
            Box::new(self.queue.items.iter().enumerate())
        } else if self.queue.marked().is_empty() {
            if let Some((idx, item)) = self.queue.selected_with_idx() {
                Box::new(std::iter::once((idx, item)))
            } else {
                Box::new(std::iter::empty::<(usize, &Song)>())
            }
        } else {
            Box::new(self.queue.marked().iter().map(|idx| (*idx, &self.queue.items[*idx])))
        }
    }

    fn open_context_menu(&mut self, ctx: &Ctx) -> Result<()> {
        let selected_song = self.queue.selected().cloned();
        let selected_song_id = selected_song.as_ref().map(|s| s.id);

        let modal = MenuModal::new(ctx)
            .list_section(ctx, |mut section| {
                section.add_item("Play", move |ctx| {
                    if let Some(id) = selected_song_id {
                        ctx.command(move |client| {
                            client.play_id(id)?;
                            Ok(())
                        });
                    }
                    Ok(())
                });
                section.add_item("Show info", move |ctx| {
                    if let Some(song) = selected_song {
                        modal!(
                            ctx,
                            InfoListModal::builder()
                                .items(&song)
                                .title("Song info")
                                .column_widths(&[30, 70])
                                .build()
                        );
                    }
                    Ok(())
                });
                Some(section)
            })
            .list_section(ctx, |mut section| {
                let items = self.queue.items.iter().map(|song| song.file.clone()).collect_vec();
                section.add_item("Add queue to playlist", |ctx| {
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
                                    client.add_to_playlist_multiple(&selected, items)?;
                                    Ok(())
                                });
                                Ok(())
                            })
                            .build()
                    );
                    Ok(())
                });
                section.add_item("Save queue as playlist", move |ctx| {
                    modal!(
                        ctx,
                        InputModal::new(ctx)
                            .title("Create new playlist")
                            .confirm_label("Save")
                            .input_label("Playlist name:")
                            .on_confirm(move |ctx, value| {
                                let value = value.to_owned();
                                ctx.command(move |client| {
                                    client.save_queue_as_playlist(&value, None)?;
                                    Ok(())
                                });
                                Ok(())
                            })
                    );
                    Ok(())
                });

                Some(section)
            })
            .list_section(ctx, |section| {
                let section = section
                    .item("Remove", move |ctx| {
                        if let Some(id) = selected_song_id {
                            ctx.command(move |client| {
                                client.delete_id(id)?;
                                Ok(())
                            });
                        }
                        Ok(())
                    })
                    .item("Clear queue", |ctx| {
                        ctx.command(|client| {
                            client.clear()?;
                            Ok(())
                        });
                        Ok(())
                    });
                Some(section)
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

impl Pane for QueuePane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> anyhow::Result<()> {
        let Ctx { config, .. } = ctx;
        self.calculate_areas(area, ctx)?;

        let filter_text = self
            .queue
            .filter()
            .map(|v| format!("[FILTER]: {v}{} ", if self.filter_input_mode { "█" } else { "" }));

        let table_block = {
            let border_style = config.as_border_style();
            let mut b = Block::default().border_style(border_style);
            if config.theme.show_song_table_header {
                b = b.borders(Borders::TOP);
            }
            if self.areas[Areas::FilterArea].height == 0
                && let Some(ref title) = filter_text
            {
                b = b.title(title.clone().set_style(border_style));
            }
            b
        };

        self.queue.state.set_content_and_viewport_len(
            self.queue.len(),
            self.areas[Areas::Table].height as usize,
        );

        let widths = Layout::horizontal(self.column_widths.as_slice())
            .flex(Flex::Start)
            .spacing(1)
            .split(self.areas[Areas::Table]);

        let formats = &config.theme.song_table_format;

        let offset = self.queue.state.as_render_state_ref().offset();
        let viewport_len = self.queue.state.viewport_len().unwrap_or_default();

        let marker_symbol_len = config.theme.symbols.marker.chars().count();
        let mut table_iter = self.queue.items.iter().enumerate().peekable();
        let mut table_items = Vec::new();

        while let Some((idx, song)) = table_iter.next() {
            // Supply default row to skip unnecessary work for rows that are either below or
            // above the visible portion of the table
            if idx < offset || idx > viewport_len + offset {
                table_items.push(Row::new((0..formats.len()).map(|_| Cell::default())));
                continue;
            }

            let is_current = ctx
                .find_current_song_in_queue()
                .map(|(_, song)| song.id)
                .is_some_and(|v| v == song.id);

            let is_marked = self.queue.marked().contains(&idx);
            let columns = (0..formats.len()).map(|i| {
                let mut max_len: usize = widths[i].width.into();
                // We have to subtract marker symbol length from max len in order to make space
                // for the marker symbol in case we are in the first column of the table and the
                // song is marked.
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
                        ctx,
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

            let is_matching_search = is_current
                || self.queue.filter().is_some_and(|filter| {
                    song.matches(self.column_formats.as_slice(), filter, ctx)
                });

            let mut row = QueueRow::default();
            if is_matching_search {
                row.cell_style = Some(config.theme.highlighted_item_style);
            }

            if matches!(ctx.config.theme.song_table_album_separator, AlbumSeparator::Underline) {
                let is_new_album = if let Some((_, next_song)) = table_iter.peek() {
                    next_song.metadata.get("album") != song.metadata.get("album")
                        || next_song.metadata.get("album_artist")
                            != song.metadata.get("album_artist")
                } else {
                    false
                };
                row.underlined = is_new_album;
            }

            table_items.push(row.into_row(columns));
        }

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
            self.queue.state.as_render_state_ref(),
        );

        if let Some(scrollbar) = config.as_styled_scrollbar()
            && self.areas[Areas::Scrollbar].width > 0
        {
            frame.render_stateful_widget(
                scrollbar,
                self.areas[Areas::Scrollbar],
                self.queue.state.as_scrollbar_state_ref(),
            );
        }

        if let Some(filter_text) = filter_text
            && self.areas[Areas::FilterArea].height > 0
        {
            frame.render_widget(
                Text::from(filter_text).style(
                    config.theme.text_color.map(|c| Style::default().fg(c)).unwrap_or_default(),
                ),
                self.areas[Areas::FilterArea],
            );
        }

        Ok(())
    }

    fn calculate_areas(&mut self, area: Rect, ctx: &Ctx) -> Result<()> {
        let Ctx { config, .. } = ctx;

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

        let table_area =
            if self.queue.filter().is_some() && !ctx.config.theme.show_song_table_header {
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

    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        self.queue.state.set_content_and_viewport_len(
            self.queue.len(),
            self.areas[Areas::Table].height as usize,
        );

        if self.should_center_cursor_on_current {
            let to_select = ctx
                .find_current_song_in_queue()
                .or(self.queue.selected_with_idx())
                .map(|(idx, _)| idx)
                .or(Some(0));
            self.queue.select_idx_opt(to_select, usize::MAX);
            self.should_center_cursor_on_current = false;
        } else {
            let to_select = self
                .queue
                .selected_with_idx()
                .or(ctx.find_current_song_in_queue())
                .map(|v| v.0)
                .or(Some(0));
            self.queue.select_idx_opt(to_select, usize::MAX);
        }

        Ok(())
    }

    fn resize(&mut self, _area: Rect, ctx: &Ctx) -> Result<()> {
        self.queue.state.set_content_and_viewport_len(
            self.queue.len(),
            self.areas[Areas::Table].height as usize,
        );
        let to_select = self
            .queue
            .selected_with_idx()
            .or(ctx.find_current_song_in_queue())
            .map(|v| v.0)
            .or(Some(0));
        self.queue.select_idx_opt(to_select, ctx.config.scrolloff);
        ctx.render()?;
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::QueueChanged => {
                self.queue.items.clone_from(&ctx.queue);
            }
            UiEvent::SongChanged => {
                if let Some((idx, _)) = ctx.find_current_song_in_queue()
                    && ctx.config.select_current_song_on_change
                {
                    match (is_visible, ctx.config.center_current_song_on_change) {
                        (true, true) => {
                            self.queue.select_idx(idx, usize::MAX);
                        }
                        (false, true) => {
                            self.queue.select_idx(idx, usize::MAX);
                            self.should_center_cursor_on_current = true;
                        }
                        (true, false) | (false, false) => {
                            self.queue.select_idx(idx, ctx.config.scrolloff);
                        }
                    }

                    ctx.render()?;
                }
            }
            UiEvent::Reconnected => {
                self.before_show(ctx)?;
            }
            UiEvent::ConfigChanged => {
                let (header, column_widths, column_formats) = Self::init(ctx);
                self.header = header;
                self.column_formats = column_formats;
                self.column_widths = column_widths;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        let position = event.into();

        if let Some(scrollbar_area) = self.scrollbar_area()
            && ctx.config.theme.scrollbar.is_some()
            && matches!(event.kind, MouseEventKind::LeftClick | MouseEventKind::Drag { .. })
            && let Some(perc) = calculate_scrollbar_position(event, scrollbar_area)
        {
            self.queue.state.scroll_to(perc, ctx.config.scrolloff);
            ctx.render()?;
            return Ok(());
        }

        if !self.areas[Areas::Table].contains(position) {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::LeftClick => {
                let clicked_row: usize = event.y.saturating_sub(self.areas[Areas::Table].y).into();
                if let Some(idx) = self.queue.state.get_at_rendered_row(clicked_row) {
                    self.queue.select_idx(idx, ctx.config.scrolloff);

                    ctx.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                let clicked_row: usize = event.y.saturating_sub(self.areas[Areas::Table].y).into();

                if let Some(song) = self
                    .queue
                    .state
                    .get_at_rendered_row(clicked_row)
                    .and_then(|idx| self.queue.items.get(idx))
                {
                    let id = song.id;
                    ctx.command(move |client| {
                        client.play_id(id)?;
                        Ok(())
                    });
                }
            }
            MouseEventKind::MiddleClick => {
                let clicked_row: usize = event.y.saturating_sub(self.areas[Areas::Table].y).into();

                if let Some(selected_song) = self
                    .queue
                    .state
                    .get_at_rendered_row(clicked_row)
                    .and_then(|idx| self.queue.items.get(idx))
                {
                    let id = selected_song.id;
                    ctx.command(move |client| {
                        client.delete_id(id)?;
                        Ok(())
                    });
                }
            }
            MouseEventKind::ScrollDown => {
                self.queue.scroll_down(1, ctx.config.scrolloff);
                ctx.render()?;
            }
            MouseEventKind::ScrollUp => {
                self.queue.scroll_up(1, ctx.config.scrolloff);
                ctx.render()?;
            }
            MouseEventKind::RightClick => {
                let clicked_row: usize = event.y.saturating_sub(self.areas[Areas::Table].y).into();
                if let Some(idx) = self.queue.state.get_at_rendered_row(clicked_row) {
                    self.queue.select_idx(idx, ctx.config.scrolloff);

                    ctx.render()?;
                }
                self.open_context_menu(ctx)?;
            }
            MouseEventKind::Drag { .. } => {}
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
            (ADD_TO_PLAYLIST, MpdQueryResult::AddToPlaylist { playlists, song_file }) => {
                modal!(
                    ctx,
                    SelectModal::builder()
                        .ctx(ctx)
                        .options(playlists)
                        .confirm_label("Add")
                        .title("Select a playlist")
                        .on_confirm(move |ctx, selected, _idx| {
                            let song_file = song_file.clone();
                            ctx.command(move |client| {
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
            (
                ADD_TO_PLAYLIST_MULTIPLE,
                MpdQueryResult::AddToPlaylistMultiple { playlists, song_files },
            ) => {
                modal!(
                    ctx,
                    SelectModal::builder()
                        .ctx(ctx)
                        .options(playlists)
                        .confirm_label("Add")
                        .title("Select a playlist")
                        .on_confirm(move |ctx, selected, _idx| {
                            ctx.command(move |client| {
                                let songs_len = song_files.len();
                                for song_file in song_files {
                                    if song_file.starts_with('/') {
                                        client.add_to_playlist(
                                            &selected,
                                            &format!("file://{song_file}"),
                                            None,
                                        )?;
                                    } else {
                                        client.add_to_playlist(&selected, &song_file, None)?;
                                    }
                                }
                                status_info!("{} songs added to playlist {}", songs_len, selected);
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

    fn handle_action(&mut self, event: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        if self.filter_input_mode {
            match event.as_common_action(ctx) {
                Some(CommonAction::Confirm) => {
                    self.filter_input_mode = false;

                    ctx.render()?;
                }
                Some(CommonAction::Close) => {
                    self.filter_input_mode = false;
                    self.queue.set_filter(None, self.column_formats.as_slice(), ctx);

                    ctx.render()?;
                }
                _ => {
                    event.stop_propagation();
                    match event.code() {
                        KeyCode::Char(c) => {
                            self.queue.push_filter(c, self.column_formats.as_slice(), ctx);
                            self.queue.jump_first_matching(self.column_formats.as_slice(), ctx);
                            ctx.render()?;
                        }
                        KeyCode::Backspace => {
                            self.queue.pop_filter(self.column_formats.as_slice(), ctx);
                            ctx.render()?;
                        }
                        _ => {}
                    }
                }
            }
        } else if let Some(action) = event.as_queue_action(ctx) {
            match action {
                QueueActions::Delete if !self.queue.marked().is_empty() => {
                    for range in self.queue.marked().ranges().rev() {
                        ctx.command(move |client| {
                            client.delete_from_queue(range.into())?;
                            Ok(())
                        });
                    }
                    self.queue.marked_mut().clear();
                    status_info!("Marked songs removed from queue");
                    ctx.render()?;
                }
                QueueActions::Delete => {
                    if let Some(selected_song) = self.queue.selected() {
                        let id = selected_song.id;
                        ctx.command(move |client| {
                            client.delete_id(id)?;
                            Ok(())
                        });
                    } else {
                        status_error!("No song selected");
                    }
                }
                QueueActions::DeleteAll => {
                    modal!(
                        ctx,
                        ConfirmModal::builder().ctx(ctx)
                            .message("Are you sure you want to clear the queue? This action cannot be undone.")
                            .on_confirm(|ctx| {
                                ctx.command(|client| Ok(client.clear()?));
                                Ok(())
                            })
                            .confirm_label("Clear")
                            .size((45, 6))
                            .build()
                    );
                }
                QueueActions::Play => {
                    if let Some(selected_song) = self.queue.selected() {
                        let id = selected_song.id;
                        ctx.command(move |client| {
                            client.play_id(id)?;
                            Ok(())
                        });
                    }
                }
                QueueActions::JumpToCurrent => {
                    if let Some((idx, _)) = ctx.status.songid.and_then(|id| {
                        self.queue.items.iter().enumerate().find(|(_, song)| song.id == id)
                    }) {
                        self.queue.select_idx(idx, ctx.config.scrolloff);
                        ctx.render()?;
                    } else {
                        status_info!("No song is currently playing");
                    }
                }
                QueueActions::Save => {
                    modal!(
                        ctx,
                        InputModal::new(ctx)
                            .title("Save queue as playlist")
                            .confirm_label("Save")
                            .input_label("Playlist name:")
                            .on_confirm(move |ctx, value| {
                                let value = value.to_owned();
                                ctx.command(move |client| {
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
                QueueActions::AddToPlaylist if !self.queue.marked().is_empty() => {
                    let mut selected_uris: Vec<String> = Vec::new();

                    self.queue.marked().ranges().for_each(|r| {
                        let sor: SingleOrRange = r.into();

                        if let Some(end) = sor.end {
                            for idx in sor.start..end {
                                if let Some(marked_song) = self.queue.items.get(idx) {
                                    selected_uris.push(marked_song.file.clone());
                                }
                            }
                        } else if let Some(marked_song) = self.queue.items.get(sor.start) {
                            selected_uris.push(marked_song.file.clone());
                        }
                    });

                    ctx.query().id(ADD_TO_PLAYLIST_MULTIPLE).target(PaneType::Queue).query(
                        move |client| {
                            let playlists = client
                                .list_playlists()?
                                .into_iter()
                                .map(|v| v.name)
                                .sorted()
                                .collect_vec();

                            Ok(MpdQueryResult::AddToPlaylistMultiple {
                                playlists,
                                song_files: selected_uris,
                            })
                        },
                    );
                }
                QueueActions::AddToPlaylist => {
                    if let Some(selected_song) = self.queue.selected() {
                        let uri = selected_song.file.clone();
                        ctx.query()
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
                QueueActions::Shuffle if !self.queue.marked().is_empty() => {
                    for range in self.queue.marked().ranges().rev() {
                        ctx.command(move |client| {
                            client.shuffle(Some(range.into()))?;
                            Ok(())
                        });
                    }
                    status_info!("Shuffled selected songs");
                }
                QueueActions::Shuffle => {
                    ctx.command(move |client| {
                        client.shuffle(None)?;
                        Ok(())
                    });
                    status_info!("Shuffled the queue");
                }
                QueueActions::Unused => {}
            }
        } else if let Some(action) = event.as_common_action(ctx).map(|v| v.to_owned()) {
            match action {
                CommonAction::Up => {
                    if !self.queue.is_empty() {
                        self.queue.prev(ctx.config.scrolloff, ctx.config.wrap_navigation);
                    }

                    ctx.render()?;
                }
                CommonAction::Down => {
                    if !self.queue.is_empty() {
                        self.queue.next(ctx.config.scrolloff, ctx.config.wrap_navigation);
                    }

                    ctx.render()?;
                }
                CommonAction::MoveUp if !self.queue.marked().is_empty() => {
                    if self.queue.is_empty() {
                        return Ok(());
                    }

                    if let Some(0) = self.queue.marked().first() {
                        return Ok(());
                    }

                    let ranges = self.queue.marked().ranges().collect_vec();
                    for range in ranges {
                        for idx in range.clone() {
                            let new_idx = idx.saturating_sub(1);
                            self.queue.items.swap(idx, new_idx);
                        }

                        let new_start_idx = range.start().saturating_sub(1);
                        ctx.command(move |client| {
                            client.move_in_queue(
                                range.into(),
                                QueuePosition::Absolute(new_start_idx),
                            )?;
                            Ok(())
                        });
                    }

                    if let Some(start) = self.queue.marked().first() {
                        let new_idx = start.saturating_sub(1);
                        self.queue.select_idx(new_idx, ctx.config.scrolloff);
                    }

                    let mut new_marked =
                        self.queue.marked().iter().map(|i| i.saturating_sub(1)).collect();
                    std::mem::swap(self.queue.marked_mut(), &mut new_marked);

                    ctx.render()?;
                    return Ok(());
                }
                CommonAction::MoveDown if !self.queue.marked().is_empty() => {
                    if self.queue.is_empty() {
                        return Ok(());
                    }

                    if let Some(last_idx) = self.queue.marked().last()
                        && *last_idx == self.queue.len() - 1
                    {
                        return Ok(());
                    }

                    let ranges = self.queue.marked().ranges().rev().collect_vec();
                    for range in ranges {
                        for idx in range.clone().rev() {
                            let new_idx = idx.saturating_add(1);
                            self.queue.items.swap(idx, new_idx);
                        }

                        let new_start_idx = range.start().saturating_add(1);
                        ctx.command(move |client| {
                            client.move_in_queue(
                                range.into(),
                                QueuePosition::Absolute(new_start_idx),
                            )?;
                            Ok(())
                        });
                    }

                    if let Some(start) = self.queue.marked().last() {
                        let new_idx = start.saturating_add(1);
                        self.queue.select_idx(new_idx, ctx.config.scrolloff);
                    }

                    let mut new_marked =
                        self.queue.marked().iter().map(|i| i.saturating_add(1)).collect();
                    std::mem::swap(self.queue.marked_mut(), &mut new_marked);

                    ctx.render()?;
                    return Ok(());
                }
                CommonAction::MoveUp => {
                    if self.queue.is_empty() {
                        return Ok(());
                    }

                    let Some((idx, selected)) = self.queue.selected_with_idx() else {
                        return Ok(());
                    };

                    let new_idx = idx.saturating_sub(1);
                    let id = selected.id;
                    ctx.command(move |client| {
                        client.move_id(id, QueuePosition::Absolute(new_idx))?;
                        Ok(())
                    });
                    self.queue.select_idx(new_idx, ctx.config.scrolloff);
                    self.queue.items.swap(idx, new_idx);
                    ctx.render()?;
                }
                CommonAction::MoveDown => {
                    if self.queue.is_empty() {
                        return Ok(());
                    }

                    let Some((idx, selected)) = self.queue.selected_with_idx() else {
                        return Ok(());
                    };

                    let new_idx = (idx + 1).min(self.queue.len() - 1);
                    let id = selected.id;
                    ctx.command(move |client| {
                        client.move_id(id, QueuePosition::Absolute(new_idx))?;
                        Ok(())
                    });
                    self.queue.select_idx(new_idx, ctx.config.scrolloff);
                    self.queue.items.swap(idx, new_idx);
                    ctx.render()?;
                }
                CommonAction::DownHalf => {
                    if !self.queue.is_empty() {
                        self.queue.next_half_viewport(ctx.config.scrolloff);
                    }

                    ctx.render()?;
                }
                CommonAction::UpHalf => {
                    if !self.queue.is_empty() {
                        self.queue.prev_half_viewport(ctx.config.scrolloff);
                    }

                    ctx.render()?;
                }
                CommonAction::PageDown => {
                    if !self.queue.is_empty() {
                        self.queue.next_viewport(ctx.config.scrolloff);
                    }

                    ctx.render()?;
                }
                CommonAction::PageUp => {
                    if !self.queue.is_empty() {
                        self.queue.prev_viewport(ctx.config.scrolloff);
                    }

                    ctx.render()?;
                }
                CommonAction::Bottom => {
                    if !self.queue.is_empty() {
                        self.queue.last();
                    }

                    ctx.render()?;
                }
                CommonAction::Top => {
                    if !self.queue.is_empty() {
                        self.queue.first();
                    }

                    ctx.render()?;
                }
                CommonAction::Right => {}
                CommonAction::Left => {}
                CommonAction::EnterSearch => {
                    self.filter_input_mode = true;
                    self.queue.set_filter(Some(String::new()), self.column_formats.as_slice(), ctx);

                    ctx.render()?;
                }
                CommonAction::NextResult => {
                    self.queue.jump_next_matching(self.column_formats.as_slice(), ctx);

                    ctx.render()?;
                }
                CommonAction::PreviousResult => {
                    self.queue.jump_previous_matching(self.column_formats.as_slice(), ctx);

                    ctx.render()?;
                }
                CommonAction::Select => {
                    if self.queue.selected().is_some() {
                        self.queue.toggle_mark_selected();
                        self.queue.next(ctx.config.scrolloff, ctx.config.wrap_navigation);

                        ctx.render()?;
                    }
                }
                CommonAction::InvertSelection => {
                    self.queue.invert_marked();

                    ctx.render()?;
                }
                CommonAction::Close if !self.queue.marked().is_empty() => {
                    self.queue.marked_mut().clear();
                    ctx.render()?;
                }
                CommonAction::AddOptions { kind: AddKind::Action(options) } => {
                    let (enqueue, _hovered_song_idx) = self.enqueue_items(options.all);

                    if !enqueue.is_empty() {
                        ctx.command(move |client| {
                            client.enqueue_multiple(enqueue, options.position, Autoplay::None)?;

                            Ok(())
                        });
                        self.queue.marked_mut().clear();
                    }
                }
                CommonAction::AddOptions { kind: AddKind::Modal(items) } => {
                    let opts = items
                        .into_iter()
                        .map(|(label, mut opts)| {
                            opts.autoplay = AutoplayKind::None;
                            let (enqueue, hovered_song_idx) = self.enqueue_items(opts.all);
                            (label, opts, (enqueue, hovered_song_idx))
                        })
                        .collect_vec();

                    modal!(ctx, create_add_modal(opts, ctx));
                    self.queue.marked_mut().clear();
                }
                CommonAction::ShowInfo => {
                    if let Some(selected_song) = self.queue.selected() {
                        modal!(
                            ctx,
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
                CommonAction::ContextMenu => {
                    self.open_context_menu(ctx)?;
                }
                CommonAction::Rate {
                    kind: RateKind::Value(value),
                    current: false,
                    min_rating: _,
                    max_rating: _,
                } => {
                    let items = self.enqueue_items(false).0;
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
                    let items = self.enqueue_items(false).0;
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
                    let items = self.enqueue_items(false).0;
                    ctx.command(move |client| {
                        client.set_sticker_multiple(LIKE_STICKER, "2".to_string(), items)?;
                        Ok(())
                    });
                }
                CommonAction::Rate { kind: RateKind::Neutral(), current: false, .. } => {
                    let items = self.enqueue_items(false).0;
                    ctx.command(move |client| {
                        client.set_sticker_multiple(LIKE_STICKER, "1".to_string(), items)?;
                        Ok(())
                    });
                }
                CommonAction::Rate { kind: RateKind::Dislike(), current: false, .. } => {
                    let items = self.enqueue_items(false).0;
                    ctx.command(move |client| {
                        client.set_sticker_multiple(LIKE_STICKER, "0".to_string(), items)?;
                        Ok(())
                    });
                }
                CommonAction::Rate { kind: _, current: true, min_rating: _, max_rating: _ } => {
                    event.abandon();
                }
                CommonAction::Save { kind: SaveKind::Playlist { name, all } } => {
                    let song_paths: Vec<String> =
                        self.items(all).map(|(_, song)| song.file.clone()).collect();
                    if song_paths.is_empty() {
                        status_warn!("No songs selected to save");
                        return Ok(());
                    }
                    ctx.command(move |client| {
                        client.add_to_playlist_multiple(&name, song_paths)?;
                        Ok(())
                    });
                }
                CommonAction::Save { kind: SaveKind::Modal { all } } => {
                    let song_paths: Vec<String> =
                        self.items(all).map(|(_, song)| song.file.clone()).collect();
                    if song_paths.is_empty() {
                        status_warn!("No songs selected to save");
                        return Ok(());
                    }
                    let modal = create_save_modal(song_paths, None, ctx)?;
                    modal!(ctx, modal);
                }
            }
        } else if let Some(action) = event.as_global_action(ctx) {
            match action {
                GlobalAction::ExternalCommand { command, .. } => {
                    let songs =
                        create_env(ctx, self.items(false).map(|(_, song)| song.file.as_str()));
                    run_external(command.clone(), songs);
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
    fn scrollbar_area(&self) -> Option<Rect> {
        let area = self.areas[Areas::Scrollbar];
        if area.width > 0 { Some(area) } else { None }
    }
}

#[derive(Default)]
struct QueueRow {
    cell_style: Option<Style>,
    underlined: bool,
}

impl QueueRow {
    fn into_row<'a>(self, cells: impl Iterator<Item = Line<'a>>) -> Row<'a> {
        let mut row = if let Some(style) = self.cell_style {
            Row::new(cells.map(|column| column.patch_style(style)))
        } else {
            Row::new(cells)
        };

        if self.underlined {
            row = row.style(self.cell_style.unwrap_or_default().underlined());
        }

        row
    }
}
