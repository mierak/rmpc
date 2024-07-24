use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use itertools::Itertools;

use crate::{
    config::{
        keys::QueueActions,
        theme::{
            properties::{Property, SongProperty},
            Position,
        },
        Config,
    },
    mpd::{
        commands::{Song, Status},
        mpd_client::{MpdClient, QueueMoveTarget},
    },
    ui::{
        modals::{
            add_to_playlist::AddToPlaylistModal, confirm_queue_clear::ConfirmQueueClearModal,
            save_queue::SaveQueueModal,
        },
        utils::dirstack::DirState,
        widgets::kitty_image::{ImageState, KittyImage},
        KeyHandleResultInternal, UiEvent,
    },
    utils::macros::{status_error, status_warn, try_ret},
    AppEvent,
};
use log::error;
use ratatui::{
    prelude::{Constraint, Layout, Rect},
    style::Stylize,
    text::Line,
    widgets::{Block, Borders, Padding, Row, Table, TableState},
    Frame,
};

use super::{CommonAction, Screen};

#[derive(Debug)]
pub struct QueueScreen {
    img_state: ImageState,
    scrolling_state: DirState<TableState>,
    filter: Option<String>,
    filter_input_mode: bool,
    queue: Vec<Song>,
    album_art: Option<Vec<u8>>,
    header: Vec<&'static str>,
    column_widths: Vec<Constraint>,
    column_formats: Vec<&'static Property<'static, SongProperty>>,
    last_song_id: Option<u32>,
}

impl QueueScreen {
    pub fn new(config: &Config, app_event_sender: std::sync::mpsc::Sender<AppEvent>) -> Self {
        Self {
            img_state: ImageState::new(app_event_sender, config.theme.default_album_art.clone()),
            scrolling_state: DirState::default(),
            filter: None,
            filter_input_mode: false,
            header: config.theme.song_table_format.iter().map(|v| v.label).collect_vec(),
            queue: Vec::new(),
            album_art: None,
            last_song_id: None,
            column_widths: config
                .theme
                .song_table_format
                .iter()
                .map(|v| Constraint::Percentage(v.width_percent))
                .collect_vec(),
            column_formats: config.theme.song_table_format.iter().map(|v| v.prop).collect_vec(),
        }
    }
}

impl Screen for QueueScreen {
    type Actions = QueueActions;
    fn render(&mut self, frame: &mut Frame, area: Rect, status: &Status, config: &Config) -> anyhow::Result<()> {
        let queue_len = self.queue.len();
        let album_art_width = config.theme.album_art_width_percent;
        let show_image = album_art_width > 0;

        let mut img_queue_constraints = [
            Constraint::Percentage(album_art_width),
            Constraint::Percentage(100 - album_art_width),
        ];

        if matches!(config.theme.album_art_position, Position::Right) {
            img_queue_constraints.reverse();
        }

        let [mut img_section, mut queue_section] = *Layout::horizontal(img_queue_constraints).split(area) else {
            return Ok(());
        };

        if matches!(config.theme.album_art_position, Position::Right) {
            std::mem::swap(&mut img_section, &mut queue_section);
        }

        let header_height = u16::from(config.theme.show_song_table_header);
        let [table_header_section, mut queue_section] =
            *Layout::vertical([Constraint::Min(header_height), Constraint::Percentage(100)]).split(queue_section)
        else {
            return Ok(());
        };

        self.scrolling_state.set_viewport_len(Some(queue_section.height.into()));
        self.scrolling_state.set_content_len(Some(queue_len));

        let widths = Layout::horizontal(self.column_widths.clone()).split(table_header_section);
        let formats = &config.theme.song_table_format;

        let table_items = self
            .queue
            .iter()
            .map(|song| {
                let is_current = status.songid.as_ref().is_some_and(|v| *v == song.id);
                let columns = (0..formats.len()).map(|i| {
                    song.as_line_ellipsized(formats[i].prop, widths[i].width.into())
                        .alignment(formats[i].alignment.into())
                });

                let is_highlighted = is_current
                    || self
                        .filter
                        .as_ref()
                        .is_some_and(|filter| song.matches(self.column_formats.as_slice(), filter, true));

                if is_highlighted {
                    Row::new(columns.map(|column| column.patch_style(config.theme.highlighted_item_style)))
                        .style(config.theme.highlighted_item_style)
                } else {
                    Row::new(columns)
                }
            })
            .collect_vec();

        let mut table_padding = Padding::right(2);
        table_padding.left = 1;
        if config.theme.show_song_table_header {
            let header_table = Table::default()
                .header(Row::new(self.header.iter().enumerate().map(|(idx, title)| {
                    Line::from(*title).alignment(formats[idx].alignment.into())
                })))
                .style(config.as_text_style())
                .widths(self.column_widths.clone())
                .block(config.as_header_table_block().padding(table_padding));
            frame.render_widget(header_table, table_header_section);
        }

        let title = self.filter.as_ref().map(|v| format!("[FILTER]: {v} "));
        let table = Table::new(table_items, self.column_widths.clone())
            .block({
                let mut b = Block::default()
                    .padding(table_padding)
                    .border_style(config.as_border_style().bold());
                if config.theme.show_song_table_header {
                    b = b.borders(Borders::TOP);
                }
                if let Some(ref title) = title {
                    b = b.title(title.clone().blue());
                }
                b
            })
            .style(config.as_text_style())
            .highlight_style(config.theme.current_item_style);

        frame.render_stateful_widget(table, queue_section, self.scrolling_state.as_render_state_ref());

        if config.theme.show_song_table_header {
            queue_section.y = queue_section.y.saturating_add(1);
            queue_section.height = queue_section.height.saturating_sub(1);
        }
        frame.render_stateful_widget(
            config.as_styled_scrollbar(),
            queue_section,
            self.scrolling_state.as_scrollbar_state_ref(),
        );
        if show_image {
            frame.render_stateful_widget(
                KittyImage::default().block(Block::default().border_style(config.as_border_style())),
                img_section,
                &mut self.img_state,
            );
        }

        Ok(())
    }

    fn before_show(&mut self, client: &mut impl MpdClient, status: &mut Status, config: &Config) -> Result<()> {
        let queue = client.playlist_info()?;
        if self.last_song_id.is_none() {
            self.last_song_id = status.songid;
        }
        self.album_art = if let Some(song) = queue
            .as_ref()
            .and_then(|p| p.iter().find(|s| status.songid.is_some_and(|i| i == s.id)))
        {
            if self.album_art.is_none() {
                log::debug!(file = song.file.as_str(); "Trying to find album art for current song for for first display");
                client.find_album_art(&song.file)?
            } else {
                Some(Vec::new())
            }
        } else {
            None
        };

        let album_art_width = config.theme.album_art_width_percent;
        let show_image = album_art_width > 0;
        if show_image && !self.album_art.as_ref().is_some_and(Vec::is_empty) {
            self.img_state.image(&mut self.album_art);
        }

        self.queue = queue.unwrap_or_default();
        self.scrolling_state.set_content_len(Some(self.queue.len()));
        if let Some(songid) = status.songid {
            let idx = self
                .queue
                .iter()
                .enumerate()
                .find(|(_, song)| song.id == songid)
                .map(|v| v.0);
            self.scrolling_state.select(idx);
        } else {
            self.scrolling_state.select(Some(0));
        }

        Ok(())
    }

    fn on_event(
        &mut self,
        event: &mut UiEvent,
        client: &mut impl MpdClient,
        status: &mut Status,
        config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        match event {
            UiEvent::Playlist => {
                let queue = client.playlist_info()?;
                if let Some(queue) = queue {
                    self.scrolling_state.set_content_len(Some(queue.len()));
                    self.queue = queue;
                } else {
                    self.scrolling_state.set_content_len(Some(0));
                    self.queue.clear();
                }
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            UiEvent::Player => {
                if let Some(current_song) = self.queue.iter().find(|s| status.songid.is_some_and(|i| i == s.id)) {
                    if !config.theme.album_art_width_percent != 0
                        && (self.last_song_id.is_some_and(|id| id != current_song.id) || self.last_song_id.is_none())
                    {
                        log::debug!(
                            file = current_song.file.as_str(),
                            selfid = self.last_song_id.as_ref(),
                            currentid = current_song.id;
                            "Trying to find album art for current song after a change"
                        );
                        self.album_art = try_ret!(
                            client.find_album_art(&current_song.file),
                            "Failed to get find album art"
                        );
                    }

                    let album_art_width = config.theme.album_art_width_percent;
                    let show_image = album_art_width > 0;
                    if show_image && !self.album_art.as_ref().is_some_and(Vec::is_empty) {
                        self.img_state.image(&mut self.album_art);
                    }
                };
                self.last_song_id = status.songid;
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }

    fn handle_action(
        &mut self,
        event: KeyEvent,
        client: &mut impl MpdClient,
        _status: &mut Status,
        config: &Config,
    ) -> Result<KeyHandleResultInternal> {
        if self.filter_input_mode {
            match config.keybinds.navigation.get(&event.into()) {
                Some(CommonAction::Confirm) => {
                    self.filter_input_mode = false;
                    self.jump_forward();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                Some(CommonAction::Close) => {
                    self.filter_input_mode = false;
                    self.filter = None;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                _ => match event.code {
                    KeyCode::Char(c) => {
                        if let Some(ref mut f) = self.filter {
                            f.push(c);
                        };
                        Ok(KeyHandleResultInternal::RenderRequested)
                    }
                    KeyCode::Backspace => {
                        if let Some(ref mut f) = self.filter {
                            f.pop();
                        };
                        Ok(KeyHandleResultInternal::RenderRequested)
                    }
                    _ => Ok(KeyHandleResultInternal::SkipRender),
                },
            }
        } else if let Some(action) = config.keybinds.queue.get(&event.into()) {
            match action {
                QueueActions::Delete => {
                    if let Some(selected_song) = self.scrolling_state.get_selected().and_then(|idx| self.queue.get(idx))
                    {
                        match client.delete_id(selected_song.id) {
                            Ok(()) => {}
                            Err(e) => error!("{:?}", e),
                        }
                    } else {
                        status_error!("No song selected");
                    }
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                QueueActions::DeleteAll => Ok(KeyHandleResultInternal::Modal(Some(Box::new(
                    ConfirmQueueClearModal::default(),
                )))),
                QueueActions::Play => {
                    if let Some(selected_song) = self.scrolling_state.get_selected().and_then(|idx| self.queue.get(idx))
                    {
                        client.play_id(selected_song.id)?;
                    }
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                QueueActions::Save => Ok(KeyHandleResultInternal::Modal(Some(
                    Box::new(SaveQueueModal::default()),
                ))),
                QueueActions::AddToPlaylist => {
                    if let Some(selected_song) = self.scrolling_state.get_selected().and_then(|idx| self.queue.get(idx))
                    {
                        let playlists = client
                            .list_playlists()?
                            .into_iter()
                            .map(|v| v.name)
                            .sorted()
                            .collect_vec();
                        Ok(KeyHandleResultInternal::Modal(Some(Box::new(AddToPlaylistModal::new(
                            selected_song.file.clone(),
                            playlists,
                        )))))
                    } else {
                        Ok(KeyHandleResultInternal::SkipRender)
                    }
                }
            }
        } else if let Some(action) = config.keybinds.navigation.get(&event.into()) {
            match action {
                CommonAction::Up => {
                    if !self.queue.is_empty() {
                        self.scrolling_state.prev();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    if !self.queue.is_empty() {
                        self.scrolling_state.next();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::MoveUp => {
                    if self.queue.is_empty() {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    }

                    let Some(idx) = self.scrolling_state.get_selected() else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };

                    let Some(selected) = self.scrolling_state.get_selected().and_then(|idx| self.queue.get(idx)) else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };

                    let new_idx = idx.saturating_sub(1);
                    client.move_id(selected.id, QueueMoveTarget::Absolute(new_idx))?;
                    self.scrolling_state.select(Some(new_idx));
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                CommonAction::MoveDown => {
                    if self.queue.is_empty() {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    }

                    let Some(idx) = self.scrolling_state.get_selected() else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };
                    let Some(selected) = self.scrolling_state.get_selected().and_then(|idx| self.queue.get(idx)) else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };

                    let new_idx = (idx + 1).min(self.queue.len() - 1);
                    client.move_id(selected.id, QueueMoveTarget::Absolute(new_idx))?;
                    self.scrolling_state.select(Some(new_idx));
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                CommonAction::DownHalf => {
                    if !self.queue.is_empty() {
                        self.scrolling_state.next_half_viewport();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::UpHalf => {
                    if !self.queue.is_empty() {
                        self.scrolling_state.prev_half_viewport();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Bottom => {
                    if !self.queue.is_empty() {
                        self.scrolling_state.last();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Top => {
                    if !self.queue.is_empty() {
                        self.scrolling_state.first();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Right => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Left => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::EnterSearch => {
                    self.filter_input_mode = true;
                    self.filter = Some(String::new());
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::NextResult => {
                    self.jump_forward();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::PreviousResult => {
                    self.jump_back();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Select => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Add => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Delete => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Rename => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Close => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::FocusInput => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Confirm => Ok(KeyHandleResultInternal::SkipRender), // queue has its own binding for play
            }
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

impl QueueScreen {
    pub fn jump_forward(&mut self) {
        let Some(filter) = self.filter.as_ref() else {
            status_warn!("No filter set");
            return;
        };
        let Some(selected) = self.scrolling_state.get_selected() else {
            error!(state:? = self.scrolling_state; "No song selected");
            return;
        };

        let length = self.queue.len();
        for i in selected + 1..length + selected {
            let i = i % length;
            if self.queue[i].matches(self.column_formats.as_slice(), filter, true) {
                self.scrolling_state.select(Some(i));
                break;
            }
        }
    }

    pub fn jump_back(&mut self) {
        let Some(filter) = self.filter.as_ref() else {
            status_warn!("No filter set");
            return;
        };
        let Some(selected) = self.scrolling_state.get_selected() else {
            error!(state:? = self.scrolling_state; "No song selected");
            return;
        };

        let length = self.queue.len();
        for i in (0..length).rev() {
            let i = (i + selected) % length;
            if self.queue[i].matches(self.column_formats.as_slice(), filter, true) {
                self.scrolling_state.select(Some(i));
                break;
            }
        }
    }
}
