use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use itertools::Itertools;
use strum::Display;

use crate::{
    config::{ui::Position, Config},
    mpd::{
        client::Client,
        mpd_client::{MpdClient, QueueMoveTarget},
    },
    state::PlayListInfoExt,
    ui::{
        modals::{
            add_to_playlist::AddToPlaylistModal, confirm_queue_clear::ConfirmQueueClearModal,
            save_queue::SaveQueueModal, Modals,
        },
        utils::dirstack::DirState,
        widgets::kitty_image::{ImageState, KittyImage},
        KeyHandleResultInternal, SharedUiState,
    },
    utils::macros::status_error,
};
use log::error;
use ratatui::{
    prelude::{Constraint, Layout, Rect},
    style::Stylize,
    text::Line,
    widgets::{Block, Borders, Padding, Row, Table, TableState},
    Frame,
};

use crate::state::State;

use super::{CommonAction, Screen};

#[derive(Debug, Default)]
pub struct QueueScreen {
    img_state: ImageState,
    scrolling_state: DirState<TableState>,
    filter: Option<String>,
    filter_input_mode: bool,
    header: Vec<&'static str>,
    column_widths: Vec<Constraint>,
}

impl QueueScreen {
    pub fn new(config: &Config) -> Self {
        Self {
            img_state: ImageState::default(),
            scrolling_state: DirState::default(),
            filter: None,
            filter_input_mode: false,
            header: config.ui.song_table_format.iter().map(|v| v.label).collect_vec(),
            column_widths: config
                .ui
                .song_table_format
                .iter()
                .map(|v| Constraint::Percentage(v.width_percent))
                .collect_vec(),
        }
    }
}

impl Screen for QueueScreen {
    type Actions = QueueActions;
    fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> anyhow::Result<()> {
        let queue_len = app.queue.len().unwrap_or(0);
        let album_art_width = app.config.ui.album_art_width_percent;
        let show_image = album_art_width > 0;

        let mut img_queue_constraints = [
            Constraint::Percentage(album_art_width),
            Constraint::Percentage(100 - album_art_width),
        ];

        if matches!(app.config.ui.album_art_position, Position::Right) {
            img_queue_constraints.reverse();
        }

        let [mut img_section, mut queue_section] = *Layout::horizontal(img_queue_constraints).split(area) else {
            return Ok(());
        };

        if matches!(app.config.ui.album_art_position, Position::Right) {
            std::mem::swap(&mut img_section, &mut queue_section);
        }

        let header_height = u16::from(app.config.ui.show_song_table_header);
        let [table_header_section, mut queue_section] =
            *Layout::vertical([Constraint::Min(header_height), Constraint::Percentage(100)]).split(queue_section)
        else {
            return Ok(());
        };

        self.scrolling_state.set_viewport_len(Some(queue_section.height.into()));
        self.scrolling_state.set_content_len(Some(queue_len));
        if show_image {
            self.img_state.image(&mut app.album_art);
        }

        let widths = Layout::horizontal(self.column_widths.clone()).split(table_header_section);
        let formats = &app.config.ui.song_table_format;

        let table_items = app
            .queue
            .as_ref()
            .map(|queue| {
                queue
                    .iter()
                    .map(|song| {
                        let is_current = app.status.songid.as_ref().is_some_and(|v| *v == song.id);
                        let mut row = Row::new((0..formats.len()).map(|i| {
                            let mut column = formats[i]
                                .prop
                                .as_line_ellipsized(song, widths[i].width.into())
                                .alignment(formats[i].alignment.into());
                            if is_current {
                                column = column.patch_style(app.config.ui.highlighted_item_style);
                            }
                            column
                        }));
                        if is_current {
                            row = row.style(app.config.ui.highlighted_item_style);
                        };
                        row
                    })
                    .collect_vec()
            })
            .unwrap_or_default();

        let mut table_padding = Padding::right(2);
        table_padding.left = 1;
        if app.config.ui.show_song_table_header {
            let header_table = Table::default()
                .header(Row::new(self.header.iter().enumerate().map(|(idx, title)| {
                    Line::from(*title).alignment(formats[idx].alignment.into())
                })))
                .widths(self.column_widths.clone())
                .block(app.config.as_header_table_block().padding(table_padding));
            frame.render_widget(header_table, table_header_section);
        }

        let title = self.filter.as_ref().map(|v| format!("[FILTER]: {v} "));
        let table = Table::new(table_items, self.column_widths.clone())
            .block({
                let mut b = Block::default()
                    .padding(table_padding)
                    .border_style(app.config.as_border_style().bold());
                if app.config.ui.show_song_table_header {
                    b = b.borders(Borders::TOP);
                }
                if let Some(ref title) = title {
                    b = b.title(title.clone().blue());
                }
                b
            })
            .highlight_style(app.config.ui.current_item_style);

        frame.render_stateful_widget(table, queue_section, self.scrolling_state.as_render_state_ref());

        queue_section.y = queue_section.y.saturating_add(1);
        queue_section.height = queue_section.height.saturating_sub(1);
        frame.render_stateful_widget(
            app.config.as_styled_scrollbar(),
            queue_section.inner(&ratatui::prelude::Margin {
                vertical: 0,
                horizontal: 0,
            }),
            self.scrolling_state.as_scrollbar_state_ref(),
        );
        if show_image {
            frame.render_stateful_widget(
                KittyImage::default()
                    .default_art(&app.config.ui.default_album_art)
                    .block(Block::default().border_style(app.config.as_border_style())),
                img_section,
                &mut self.img_state,
            );
        }

        Ok(())
    }

    fn before_show(
        &mut self,
        _client: &mut Client<'_>,
        app: &mut crate::state::State,
        _shared: &mut SharedUiState,
    ) -> Result<()> {
        self.scrolling_state.set_content_len(app.queue.len());
        if let Some(songid) = app.status.songid {
            let idx = app
                .queue
                .as_ref()
                .and_then(|queue| queue.iter().enumerate().find(|(_, song)| song.id == songid))
                .map(|v| v.0);
            self.scrolling_state.select(idx);
        } else if app.queue.len().is_some() {
            self.scrolling_state.select(Some(0));
        }

        Ok(())
    }

    fn handle_action(
        &mut self,
        event: KeyEvent,
        client: &mut Client<'_>,
        app: &mut State,
        _shared: &mut SharedUiState,
    ) -> Result<KeyHandleResultInternal> {
        if self.filter_input_mode {
            match event.code {
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
                KeyCode::Enter => {
                    self.filter_input_mode = false;
                    self.jump_forward(app);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                KeyCode::Esc => {
                    self.filter_input_mode = false;
                    self.filter = None;
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                _ => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else if let Some(action) = app.config.keybinds.queue.get(&event.into()) {
            match action {
                QueueActions::Delete => {
                    if let Some(selected_song) = app.queue.get_selected(self.scrolling_state.get_selected()) {
                        match client.delete_id(selected_song.id) {
                            Ok(()) => {}
                            Err(e) => error!("{:?}", e),
                        }
                    } else {
                        status_error!("No song selected");
                    }
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                QueueActions::DeleteAll => Ok(KeyHandleResultInternal::Modal(Some(Modals::ConfirmQueueClear(
                    ConfirmQueueClearModal::default(),
                )))),
                QueueActions::Play => {
                    if let Some(selected_song) = app.queue.get_selected(self.scrolling_state.get_selected()) {
                        client.play_id(selected_song.id)?;
                    }
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                QueueActions::Save => Ok(KeyHandleResultInternal::Modal(Some(Modals::SaveQueue(
                    SaveQueueModal::default(),
                )))),
                QueueActions::AddToPlaylist => {
                    if let Some(selected_song) = app.queue.get_selected(self.scrolling_state.get_selected()) {
                        let playlists = client
                            .list_playlists()?
                            .into_iter()
                            .map(|v| v.name)
                            .sorted()
                            .collect_vec();
                        Ok(KeyHandleResultInternal::Modal(Some(Modals::AddToPlaylist(
                            AddToPlaylistModal::new(selected_song.file.clone(), playlists),
                        ))))
                    } else {
                        Ok(KeyHandleResultInternal::SkipRender)
                    }
                }
            }
        } else if let Some(action) = app.config.keybinds.navigation.get(&event.into()) {
            match action {
                CommonAction::Up => {
                    if !app.queue.is_empty_or_none() {
                        self.scrolling_state.prev();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    if !app.queue.is_empty_or_none() {
                        self.scrolling_state.next();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::MoveUp => {
                    if app.queue.is_empty_or_none() {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    }

                    let Some(idx) = self.scrolling_state.get_selected() else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };
                    let Some(selected) = app.queue.get_selected(Some(idx)) else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };

                    let new_idx = idx.saturating_sub(1);
                    client.move_id(selected.id, QueueMoveTarget::Absolute(new_idx))?;
                    self.scrolling_state.select(Some(new_idx));
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                CommonAction::MoveDown => {
                    if app.queue.is_empty_or_none() {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    }

                    let Some(idx) = self.scrolling_state.get_selected() else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };
                    let Some(selected) = app.queue.get_selected(Some(idx)) else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };

                    let new_idx = (idx + 1).min(app.queue.len().unwrap_or(1) - 1);
                    client.move_id(selected.id, QueueMoveTarget::Absolute(new_idx))?;
                    self.scrolling_state.select(Some(new_idx));
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                CommonAction::DownHalf => {
                    if !app.queue.is_empty_or_none() {
                        self.scrolling_state.next_half_viewport();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::UpHalf => {
                    if !app.queue.is_empty_or_none() {
                        self.scrolling_state.prev_half_viewport();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Bottom => {
                    if !app.queue.is_empty_or_none() {
                        self.scrolling_state.last();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Top => {
                    if !app.queue.is_empty_or_none() {
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
                    self.jump_forward(app);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::PreviousResult => {
                    self.jump_back(app);
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
    pub fn jump_forward(&mut self, app: &mut crate::state::State) {
        if let Some(filter) = self.filter.as_ref() {
            if let Some(selected) = self.scrolling_state.get_selected() {
                for i in selected + 1..app.queue.len().unwrap_or(0) {
                    if app.queue.as_ref().is_some_and(|q| {
                        q[i].title
                            .as_ref()
                            .is_some_and(|v| v.to_lowercase().contains(&filter.to_lowercase()))
                    }) {
                        self.scrolling_state.select(Some(i));
                        break;
                    }
                }
            }
        }
    }

    pub fn jump_back(&mut self, app: &mut crate::state::State) {
        if let Some(filter) = self.filter.as_ref() {
            if let Some(selected) = self.scrolling_state.get_selected() {
                for i in (0..selected).rev() {
                    if app.queue.as_ref().is_some_and(|q| {
                        q[i].title
                            .as_ref()
                            .is_some_and(|v| v.to_lowercase().contains(&filter.to_lowercase()))
                    }) {
                        self.scrolling_state.select(Some(i));
                        break;
                    }
                }
            }
        }
    }
}

#[derive(Debug, Display, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub enum QueueActions {
    Delete,
    DeleteAll,
    Play,
    Save,
    AddToPlaylist,
}
