use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use itertools::Itertools;

use crate::{
    cli::{create_env, run_external},
    config::{
        keys::{GlobalAction, QueueActions},
        theme::{
            properties::{Property, SongProperty},
            Position,
        },
        Config,
    },
    context::AppContext,
    mpd::{
        commands::Song,
        mpd_client::{MpdClient, QueueMoveTarget},
    },
    ui::{
        image::facade::AlbumArtFacade,
        modals::{
            add_to_playlist::AddToPlaylistModal, confirm_queue_clear::ConfirmQueueClearModal,
            save_queue::SaveQueueModal,
        },
        utils::dirstack::DirState,
        KeyHandleResultInternal, UiEvent,
    },
    utils::macros::{status_error, status_warn, try_skip},
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
    scrolling_state: DirState<TableState>,
    filter: Option<String>,
    filter_input_mode: bool,
    header: Vec<&'static str>,
    column_widths: Vec<Constraint>,
    column_formats: Vec<&'static Property<'static, SongProperty>>,
    album_art_facade: AlbumArtFacade,
}

impl QueueScreen {
    pub fn new(config: &Config, context: &AppContext) -> Self {
        let sender = context.app_event_sender.clone();
        Self {
            album_art_facade: AlbumArtFacade::new(
                config.album_art.method.into(),
                config.theme.default_album_art,
                config.album_art.max_size_px,
                move |full_render: bool| {
                    try_skip!(
                        sender.send(AppEvent::RequestRender(full_render)),
                        "Failed to request render"
                    );
                },
            ),
            scrolling_state: DirState::default(),
            filter: None,
            filter_input_mode: false,
            header: config.theme.song_table_format.iter().map(|v| v.label).collect_vec(),
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
    fn render(&mut self, frame: &mut Frame, area: Rect, context: &AppContext) -> anyhow::Result<()> {
        let AppContext {
            queue, config, status, ..
        } = context;
        let queue_len = queue.len();
        let album_art_width = config.theme.album_art_width_percent;

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

        let table_items = queue
            .iter()
            .map(|song| {
                let is_current = status.songid.as_ref().is_some_and(|v| *v == song.id);
                let columns = (0..formats.len()).map(|i| {
                    song.as_line_ellipsized(formats[i].prop, widths[i].width.into())
                        .unwrap_or_default()
                        .alignment(formats[i].alignment.into())
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
        self.album_art_facade.render(frame, img_section, config)?;

        Ok(())
    }

    fn post_render(&mut self, frame: &mut Frame, context: &AppContext) -> Result<()> {
        self.album_art_facade.post_render(frame, context.config)
    }

    fn before_show(&mut self, client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        let status = &context.status;

        let album_art = if let Some(current_song) = context.queue.iter().find(|v| Some(v.id) == status.songid) {
            client.find_album_art(current_song.file.as_str())?
        } else {
            None
        };

        self.album_art_facade.set_image(album_art)?;
        self.album_art_facade.show();

        self.scrolling_state.set_content_len(Some(context.queue.len()));
        self.scrolling_state
            .select(context.find_current_song_in_queue().map(|v| v.0).or(Some(0)));

        Ok(())
    }

    fn on_hide(&mut self, _client: &mut impl MpdClient, context: &AppContext) -> Result<()> {
        self.album_art_facade.hide(context.config.theme.background_color)
    }

    fn on_event(
        &mut self,
        event: &mut UiEvent,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<KeyHandleResultInternal> {
        match event {
            UiEvent::Player => {
                if let Some((idx, current_song)) = context
                    .queue
                    .iter()
                    .enumerate()
                    .find(|(_, v)| Some(v.id) == context.status.songid)
                {
                    let album_art = client.find_album_art(current_song.file.as_str())?;
                    self.album_art_facade.set_image(album_art)?;
                    if context.config.select_current_song_on_change {
                        self.scrolling_state.select(Some(idx));
                    }
                }
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            UiEvent::Resized { columns, rows } => {
                self.album_art_facade.resize(*columns, *rows);
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            UiEvent::ModalOpened => {
                self.album_art_facade.hide(context.config.theme.background_color)?;
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            UiEvent::ModalClosed => {
                self.album_art_facade.show();
                Ok(KeyHandleResultInternal::RenderRequested)
            }
            UiEvent::Exit => {
                self.album_art_facade.cleanup()?;
                Ok(KeyHandleResultInternal::SkipRender)
            }
            _ => Ok(KeyHandleResultInternal::SkipRender),
        }
    }

    fn handle_action(
        &mut self,
        event: KeyEvent,
        client: &mut impl MpdClient,
        context: &AppContext,
    ) -> Result<KeyHandleResultInternal> {
        let config = context.config;
        if self.filter_input_mode {
            match config.keybinds.navigation.get(&event.into()) {
                Some(CommonAction::Confirm) => {
                    self.filter_input_mode = false;
                    self.jump_forward(&context.queue);
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
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                QueueActions::DeleteAll => Ok(KeyHandleResultInternal::Modal(Some(Box::new(
                    ConfirmQueueClearModal::default(),
                )))),
                QueueActions::Play => {
                    if let Some(selected_song) = self
                        .scrolling_state
                        .get_selected()
                        .and_then(|idx| context.queue.get(idx))
                    {
                        client.play_id(selected_song.id)?;
                    }
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                QueueActions::Save => Ok(KeyHandleResultInternal::Modal(Some(
                    Box::new(SaveQueueModal::default()),
                ))),
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
                    if !context.queue.is_empty() {
                        self.scrolling_state.prev();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    if !context.queue.is_empty() {
                        self.scrolling_state.next();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::MoveUp => {
                    if context.queue.is_empty() {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    }

                    let Some(idx) = self.scrolling_state.get_selected() else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };

                    let Some(selected) = self
                        .scrolling_state
                        .get_selected()
                        .and_then(|idx| context.queue.get(idx))
                    else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };

                    let new_idx = idx.saturating_sub(1);
                    client.move_id(selected.id, QueueMoveTarget::Absolute(new_idx))?;
                    self.scrolling_state.select(Some(new_idx));
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                CommonAction::MoveDown => {
                    if context.queue.is_empty() {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    }

                    let Some(idx) = self.scrolling_state.get_selected() else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };
                    let Some(selected) = self
                        .scrolling_state
                        .get_selected()
                        .and_then(|idx| context.queue.get(idx))
                    else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };

                    let new_idx = (idx + 1).min(context.queue.len() - 1);
                    client.move_id(selected.id, QueueMoveTarget::Absolute(new_idx))?;
                    self.scrolling_state.select(Some(new_idx));
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                CommonAction::DownHalf => {
                    if !context.queue.is_empty() {
                        self.scrolling_state.next_half_viewport();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::UpHalf => {
                    if !context.queue.is_empty() {
                        self.scrolling_state.prev_half_viewport();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Bottom => {
                    if !context.queue.is_empty() {
                        self.scrolling_state.last();
                    }
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Top => {
                    if !context.queue.is_empty() {
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
                    self.jump_forward(&context.queue);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::PreviousResult => {
                    self.jump_back(&context.queue);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Select => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Add => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::AddAll => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Delete => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Rename => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Close => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::FocusInput => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Confirm => Ok(KeyHandleResultInternal::SkipRender), // queue has its own binding for play
            }
        } else if let Some(action) = config.keybinds.global.get(&event.into()) {
            match action {
                GlobalAction::ExternalCommand { command, .. } => {
                    let song = self
                        .scrolling_state
                        .get_selected()
                        .and_then(|idx| context.queue.get(idx).map(|song| song.file.as_str()));

                    run_external(command, create_env(context, song, client)?);
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                _ => Ok(KeyHandleResultInternal::KeyNotHandled),
            }
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}

impl QueueScreen {
    pub fn jump_forward(&mut self, queue: &[Song]) {
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
                self.scrolling_state.select(Some(i));
                break;
            }
        }
    }

    pub fn jump_back(&mut self, queue: &[Song]) {
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
                self.scrolling_state.select(Some(i));
                break;
            }
        }
    }
}
