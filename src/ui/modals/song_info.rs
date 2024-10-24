use crate::{config::keys::CommonAction, context::AppContext, mpd::commands::Song};
use ratatui::{
    layout::{Constraint, Layout, Margin},
    style::Style,
    symbols::border,
    widgets::{Block, Borders, Cell, Clear, Row, Table, TableState},
    Frame,
};

use crate::ui::{utils::dirstack::DirState, KeyHandleResultInternal};

use super::{Modal, RectExt};

#[derive(Debug)]
pub struct SongInfoModal {
    scrolling_state: DirState<TableState>,
    song: Song,
}

impl SongInfoModal {
    pub fn new(song: Song) -> Self {
        let mut scrolling_state = DirState::default();
        scrolling_state.select(Some(0), 0);
        Self { scrolling_state, song }
    }
}

impl Modal for SongInfoModal {
    fn render(&mut self, frame: &mut Frame, app: &mut crate::context::AppContext) -> anyhow::Result<()> {
        let popup_area = frame.area().centered(80, 80);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = app.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(app.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center)
            .title("Song info");

        let [header_area, table_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Percentage(100)]).areas(popup_area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }));

        let Self { song, .. } = self;
        let mut rows = Vec::new();

        rows.push(Row::new([Cell::from("File"), Cell::from(song.file.clone())]));
        if let Some(file_name) = song.file_name() {
            rows.push(Row::new([Cell::from("Filename"), Cell::from(file_name.into_owned())]));
        };
        if let Some(title) = song.title() {
            rows.push(Row::new([Cell::from("Title"), Cell::from(title.clone())]));
        }
        if let Some(artist) = song.artist() {
            rows.push(Row::new([Cell::from("Artist"), Cell::from(artist.clone())]));
        }
        if let Some(album) = song.album() {
            rows.push(Row::new([Cell::from("Album"), Cell::from(album.clone())]));
        }
        if let Some(duration) = song.duration {
            rows.push(Row::new([
                Cell::from("Duration"),
                Cell::from(duration.as_secs().to_string()),
            ]));
        }

        for (k, v) in &song.metadata {
            rows.push(Row::new([Cell::from(k.clone()), Cell::from(v.clone())]));
        }

        self.scrolling_state.set_content_len(Some(rows.len()));
        self.scrolling_state.set_viewport_len(Some(table_area.height.into()));

        let header_table = Table::new(
            vec![Row::new([Cell::from("Tag"), Cell::from("Value")])],
            [Constraint::Percentage(30), Constraint::Percentage(70)],
        )
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(app.config.as_border_style()),
        );
        let table = Table::new(rows, [Constraint::Percentage(30), Constraint::Percentage(70)])
            .column_spacing(0)
            .style(app.config.as_text_style())
            .highlight_style(app.config.theme.current_item_style);

        frame.render_widget(block, popup_area);
        frame.render_widget(header_table, header_area);
        frame.render_stateful_widget(
            table,
            table_area.inner(Margin {
                horizontal: 1,
                vertical: 0,
            }),
            self.scrolling_state.as_render_state_ref(),
        );
        frame.render_stateful_widget(
            app.config.as_styled_scrollbar(),
            popup_area.inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
            self.scrolling_state.as_scrollbar_state_ref(),
        );

        return Ok(());
    }

    fn handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        _client: &mut crate::mpd::client::Client<'_>,
        context: &mut AppContext,
    ) -> anyhow::Result<KeyHandleResultInternal> {
        if let Some(action) = context.config.keybinds.navigation.get(&key.into()) {
            match action {
                CommonAction::DownHalf => {
                    self.scrolling_state.next_half_viewport(context.config.scrolloff);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::UpHalf => {
                    self.scrolling_state.prev_half_viewport(context.config.scrolloff);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Up => {
                    self.scrolling_state
                        .prev(context.config.scrolloff, context.config.wrap_navigation);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    self.scrolling_state
                        .next(context.config.scrolloff, context.config.wrap_navigation);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Bottom => {
                    self.scrolling_state.last();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Top => {
                    self.scrolling_state.first();
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Right => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Left => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::EnterSearch => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::NextResult => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::PreviousResult => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Add => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::AddAll => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Select => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Delete => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Rename => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::MoveUp => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::MoveDown => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::Close => Ok(KeyHandleResultInternal::Modal(None)),
                CommonAction::Confirm => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::FocusInput => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::PaneDown => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::PaneUp => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::PaneRight => Ok(KeyHandleResultInternal::SkipRender),
                CommonAction::PaneLeft => Ok(KeyHandleResultInternal::SkipRender),
            }
        } else {
            Ok(KeyHandleResultInternal::KeyNotHandled)
        }
    }
}
