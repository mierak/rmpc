use crate::{config::keys::CommonAction, context::AppContext, mpd::commands::Song, shared::macros::pop_modal};
use ratatui::{
    layout::{Constraint, Layout, Margin},
    style::Style,
    symbols::border,
    text::Text,
    widgets::{Block, Borders, Cell, Clear, Row, Table, TableState},
    Frame,
};

use crate::ui::{dirstack::DirState, KeyHandleResultInternal};

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

    #[allow(clippy::cast_possible_truncation)]
    fn row<'a>(key: &'a str, key_width: u16, value: &'a str, value_width: u16) -> Row<'a> {
        let key = textwrap::fill(key, key_width as usize);
        let value = textwrap::fill(value, value_width as usize);
        let lines = key.lines().count().max(value.lines().count());

        Row::new([Cell::from(Text::from(key)), Cell::from(Text::from(value))]).height(lines as u16)
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

        let (key_col_width, val_col_width) = (30, 70);
        let margin = Margin {
            horizontal: 1,
            vertical: 0,
        };
        let [header_area, table_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Percentage(100)]).areas(block.inner(popup_area));
        let header_area = header_area.inner(margin);
        let table_area = table_area.inner(margin);

        let [tag_area, mut value_area] = Layout::horizontal([
            Constraint::Percentage(key_col_width),
            Constraint::Percentage(val_col_width),
        ])
        .areas(table_area);
        value_area.width = value_area.width.saturating_sub(1); // account for the column spacing

        let Self { song, .. } = self;
        let mut rows = Vec::new();

        rows.push(SongInfoModal::row("File", tag_area.width, &song.file, value_area.width));
        let file_name = song.file_name().unwrap_or_default();
        if !file_name.is_empty() {
            rows.push(SongInfoModal::row(
                "Filename",
                tag_area.width,
                &file_name,
                value_area.width,
            ));
        };
        if let Some(title) = song.title() {
            rows.push(SongInfoModal::row("Title", tag_area.width, title, value_area.width));
        }
        if let Some(artist) = song.artist() {
            rows.push(SongInfoModal::row("Artist", tag_area.width, artist, value_area.width));
        }
        if let Some(album) = song.album() {
            rows.push(SongInfoModal::row("Album", tag_area.width, album, value_area.width));
        }
        let duration = song
            .duration
            .as_ref()
            .map(|d| d.as_secs().to_string())
            .unwrap_or_default();
        if !duration.is_empty() {
            rows.push(SongInfoModal::row(
                "Duration",
                tag_area.width,
                &duration,
                value_area.width,
            ));
        }

        for (k, v) in song
            .metadata
            .iter()
            .filter(|(key, _)| !["title", "album", "artist", "duration"].contains(&(*key).as_str()))
        {
            rows.push(SongInfoModal::row(k, tag_area.width, v, value_area.width));
        }

        self.scrolling_state.set_content_len(Some(rows.len()));
        self.scrolling_state.set_viewport_len(Some(table_area.height.into()));

        let header_table = Table::new(
            vec![Row::new([Cell::from("Tag"), Cell::from("Value")])],
            [
                Constraint::Percentage(key_col_width),
                Constraint::Percentage(val_col_width),
            ],
        )
        .column_spacing(1)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(app.config.as_border_style()),
        );
        let table = Table::new(
            rows,
            [
                Constraint::Percentage(key_col_width),
                Constraint::Percentage(val_col_width),
            ],
        )
        .column_spacing(1)
        .style(app.config.as_text_style())
        .row_highlight_style(app.config.theme.current_item_style);

        frame.render_widget(block, popup_area);
        frame.render_widget(header_table, header_area);
        frame.render_stateful_widget(table, table_area, self.scrolling_state.as_render_state_ref());
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

                    context.render()?;
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                CommonAction::UpHalf => {
                    self.scrolling_state.prev_half_viewport(context.config.scrolloff);

                    context.render()?;
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                CommonAction::Up => {
                    self.scrolling_state
                        .prev(context.config.scrolloff, context.config.wrap_navigation);

                    context.render()?;
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                CommonAction::Down => {
                    self.scrolling_state
                        .next(context.config.scrolloff, context.config.wrap_navigation);

                    context.render()?;
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                CommonAction::Bottom => {
                    self.scrolling_state.last();

                    context.render()?;
                    Ok(KeyHandleResultInternal::SkipRender)
                }
                CommonAction::Top => {
                    self.scrolling_state.first();

                    context.render()?;
                    Ok(KeyHandleResultInternal::SkipRender)
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
                CommonAction::Close => {
                    pop_modal!(context);
                    Ok(KeyHandleResultInternal::SkipRender)
                }
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
