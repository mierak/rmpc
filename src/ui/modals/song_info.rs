use anyhow::Result;
use itertools::Itertools;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::Style;
use ratatui::symbols::border;
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Cell, Clear, Row, Table, TableState};

use super::{Modal, RectExt};
use crate::config::keys::CommonAction;
use crate::context::AppContext;
use crate::mpd::commands::Song;
use crate::shared::key_event::KeyEvent;
use crate::shared::macros::pop_modal;
use crate::shared::mouse_event::{MouseEvent, MouseEventKind};
use crate::ui::dirstack::DirState;

#[derive(Debug)]
pub struct SongInfoModal {
    scrolling_state: DirState<TableState>,
    table_area: Rect,
    song: Song,
}

impl SongInfoModal {
    pub fn new(song: Song) -> Self {
        let mut scrolling_state = DirState::default();
        scrolling_state.select(Some(0), 0);
        Self { scrolling_state, song, table_area: Rect::default() }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn row<'a>(
        key: &'a str,
        key_width: u16,
        value: &'a str,
        value_width: u16,
    ) -> impl Iterator<Item = Row<'a>> {
        let key = textwrap::wrap(key, key_width as usize);
        let value = textwrap::wrap(value, value_width as usize);

        key.into_iter().zip_longest(value).map(|item| {
            let (key, value) = match item {
                itertools::EitherOrBoth::Both(key, value) => (Some(key), Some(value)),
                itertools::EitherOrBoth::Left(key) => (Some(key), None),
                itertools::EitherOrBoth::Right(value) => (None, Some(value)),
            };
            Row::new([
                Cell::from(Text::from(key.unwrap_or_default())),
                Cell::from(Text::from(value.unwrap_or_default())),
            ])
        })
    }
}

impl Modal for SongInfoModal {
    fn render(&mut self, frame: &mut Frame, app: &mut AppContext) -> Result<()> {
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
        let margin = Margin { horizontal: 1, vertical: 0 };
        let [header_area, table_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Percentage(100)])
                .areas(block.inner(popup_area));
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

        rows.extend(SongInfoModal::row("File", tag_area.width, &song.file, value_area.width));
        let file_name = song.file_name().unwrap_or_default();
        if !file_name.is_empty() {
            rows.extend(SongInfoModal::row(
                "Filename",
                tag_area.width,
                &file_name,
                value_area.width,
            ));
        };
        if let Some(title) = song.title() {
            rows.extend(SongInfoModal::row("Title", tag_area.width, title, value_area.width));
        }
        if let Some(artist) = song.artist() {
            rows.extend(SongInfoModal::row("Artist", tag_area.width, artist, value_area.width));
        }
        if let Some(album) = song.album() {
            rows.extend(SongInfoModal::row("Album", tag_area.width, album, value_area.width));
        }
        let duration = song.duration.as_ref().map(|d| d.as_secs().to_string()).unwrap_or_default();
        if !duration.is_empty() {
            rows.extend(SongInfoModal::row(
                "Duration",
                tag_area.width,
                &duration,
                value_area.width,
            ));
        }

        rows.extend(
            song.metadata
                .iter()
                .filter(|(key, _)| {
                    !["title", "album", "artist", "duration"].contains(&(*key).as_str())
                })
                .flat_map(|(k, v)| SongInfoModal::row(k, tag_area.width, v, value_area.width)),
        );

        self.scrolling_state.set_content_len(Some(rows.len()));
        self.scrolling_state.set_viewport_len(Some(table_area.height.into()));

        let header_table = Table::new(vec![Row::new([Cell::from("Tag"), Cell::from("Value")])], [
            Constraint::Percentage(key_col_width),
            Constraint::Percentage(val_col_width),
        ])
        .column_spacing(1)
        .block(
            Block::default().borders(Borders::BOTTOM).border_style(app.config.as_border_style()),
        );
        let table = Table::new(rows, [
            Constraint::Percentage(key_col_width),
            Constraint::Percentage(val_col_width),
        ])
        .column_spacing(1)
        .style(app.config.as_text_style())
        .row_highlight_style(app.config.theme.current_item_style);

        self.table_area = table_area;

        frame.render_widget(block, popup_area);
        frame.render_widget(header_table, header_area);
        frame.render_stateful_widget(table, table_area, self.scrolling_state.as_render_state_ref());
        frame.render_stateful_widget(
            app.config.as_styled_scrollbar(),
            popup_area.inner(Margin { horizontal: 0, vertical: 1 }),
            self.scrolling_state.as_scrollbar_state_ref(),
        );

        return Ok(());
    }

    fn handle_key(&mut self, key: &mut KeyEvent, context: &mut AppContext) -> Result<()> {
        if let Some(action) = key.as_common_action(context) {
            match action {
                CommonAction::DownHalf => {
                    self.scrolling_state.next_half_viewport(context.config.scrolloff);

                    context.render()?;
                }
                CommonAction::UpHalf => {
                    self.scrolling_state.prev_half_viewport(context.config.scrolloff);

                    context.render()?;
                }
                CommonAction::Up => {
                    self.scrolling_state
                        .prev(context.config.scrolloff, context.config.wrap_navigation);

                    context.render()?;
                }
                CommonAction::Down => {
                    self.scrolling_state
                        .next(context.config.scrolloff, context.config.wrap_navigation);

                    context.render()?;
                }
                CommonAction::Bottom => {
                    self.scrolling_state.last();

                    context.render()?;
                }
                CommonAction::Top => {
                    self.scrolling_state.first();

                    context.render()?;
                }
                CommonAction::Close => {
                    pop_modal!(context);
                }
                _ => {}
            }
        };

        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &mut AppContext) -> Result<()> {
        if !self.table_area.contains(event.into()) {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick => {
                let y: usize = event.y.saturating_sub(self.table_area.y).into();
                if let Some(idx) = self.scrolling_state.get_at_rendered_row(y) {
                    self.scrolling_state.select(Some(idx), context.config.scrolloff);
                    context.render()?;
                }
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollDown => {
                self.scrolling_state.next(context.config.scrolloff, false);
                context.render()?;
            }
            MouseEventKind::ScrollUp => {
                self.scrolling_state.prev(context.config.scrolloff, false);
                context.render()?;
            }
        }

        Ok(())
    }
}
