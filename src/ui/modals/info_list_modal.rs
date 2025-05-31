use std::time::Duration;

use anyhow::Result;
use bon::bon;
use itertools::Itertools;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::Style,
    symbols::border,
    text::Text,
    widgets::{Block, Borders, Cell, Clear, Row, Table, TableState},
};

use super::{Modal, RectExt};
use crate::{
    config::keys::CommonAction,
    context::AppContext,
    mpd::commands::Song,
    shared::{
        ext::duration::DurationExt,
        key_event::KeyEvent,
        macros::pop_modal,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::dirstack::DirState,
};

#[derive(Debug)]
pub struct InfoListModal {
    scrolling_state: DirState<TableState>,
    table_area: Rect,
    items: KeyValues,
    column_widths: &'static [u16],
    title: &'static str,
    size: (u16, u16),
}

#[derive(Debug)]
struct KeyValues(Vec<KeyValue>);
#[derive(Debug)]
struct KeyValue {
    key: String,
    value: String,
}

#[bon]
impl InfoListModal {
    #[builder]
    pub fn new(
        items: impl Into<KeyValues>,
        title: &'static str,
        column_widths: &'static [u16],
        size: Option<(u16, u16)>,
    ) -> Self {
        let mut scrolling_state = DirState::default();
        scrolling_state.select(Some(0), 0);
        Self {
            scrolling_state,
            items: items.into(),
            table_area: Rect::default(),
            title,
            column_widths,
            size: size.unwrap_or((80, 80)),
        }
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

impl Modal for InfoListModal {
    fn render(&mut self, frame: &mut Frame, app: &mut AppContext) -> Result<()> {
        let popup_area = frame.area().centered(self.size.0, self.size.1);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = app.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(app.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center)
            .title(self.title);

        let margin = Margin { horizontal: 1, vertical: 0 };
        let [header_area, table_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Percentage(100)])
                .areas(block.inner(popup_area));
        let header_area = header_area.inner(margin);
        let table_area = table_area.inner(margin);

        let column_constraints =
            self.column_widths.iter().map(|w| Constraint::Percentage(*w)).collect_vec();
        let column_areas = Layout::horizontal(&column_constraints).spacing(1).split(table_area);

        let rows = self
            .items
            .0
            .iter()
            .flat_map(|item| {
                InfoListModal::row(
                    &item.key,
                    column_areas[0].width,
                    &item.value,
                    column_areas[1].width,
                )
            })
            .collect_vec();

        self.scrolling_state.set_content_len(Some(rows.len()));
        self.scrolling_state.set_viewport_len(Some(table_area.height.into()));

        let header_table = Table::new(
            vec![Row::new([Cell::from("Tag"), Cell::from("Value")])],
            &column_constraints,
        )
        .column_spacing(1)
        .block(
            Block::default().borders(Borders::BOTTOM).border_style(app.config.as_border_style()),
        );
        let table = Table::new(rows, &column_constraints)
            .column_spacing(1)
            .style(app.config.as_text_style())
            .row_highlight_style(app.config.theme.current_item_style);

        self.table_area = table_area;

        frame.render_widget(block, popup_area);
        frame.render_widget(header_table, header_area);
        frame.render_stateful_widget(table, table_area, self.scrolling_state.as_render_state_ref());
        if let Some(scrollbar) = app.config.as_styled_scrollbar() {
            frame.render_stateful_widget(
                scrollbar,
                popup_area.inner(Margin { horizontal: 0, vertical: 1 }),
                self.scrolling_state.as_scrollbar_state_ref(),
            );
        }

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
        }

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

impl From<Vec<Song>> for KeyValues {
    fn from(value: Vec<Song>) -> Self {
        let mut result = Vec::new();

        let total_duration: Duration = value.iter().filter_map(|v| v.duration).sum();
        let total_artists = value
            .iter()
            .filter_map(|v| v.metadata.get("artist"))
            .flat_map(|tag| tag.iter())
            .unique()
            .count();
        let total_albums = value
            .iter()
            .filter_map(|v| v.metadata.get("album"))
            .flat_map(|tag| tag.iter())
            .unique()
            .count();
        let total_genres = value
            .iter()
            .filter_map(|v| v.metadata.get("genre"))
            .flat_map(|tag| tag.iter())
            .unique()
            .count();

        result.push(KeyValue { key: "Songs".to_owned(), value: value.len().to_string() });
        result
            .push(KeyValue { key: "Total duration".to_owned(), value: total_duration.to_string() });
        result.push(KeyValue { key: "Artists".to_owned(), value: total_artists.to_string() });
        result.push(KeyValue { key: "Albums".to_owned(), value: total_albums.to_string() });
        result.push(KeyValue { key: "Genres".to_owned(), value: total_genres.to_string() });
        KeyValues(result)
    }
}

impl From<&Song> for KeyValues {
    fn from(song: &Song) -> Self {
        let mut result = Vec::new();
        result.push(KeyValue { key: "File".to_owned(), value: song.file.clone() });
        let file_name = song.file_name().unwrap_or_default();
        if !file_name.is_empty() {
            result.push(KeyValue { key: "Filename".to_owned(), value: file_name.into_owned() });
        }

        if let Some(title) = song.metadata.get("title") {
            result.extend(
                title
                    .iter()
                    .map(|item| KeyValue { key: "Title".to_owned(), value: item.to_owned() }),
            );
        }

        if let Some(artist) = song.metadata.get("artist") {
            result.extend(
                artist
                    .iter()
                    .map(|item| KeyValue { key: "Artist".to_owned(), value: item.to_owned() }),
            );
        }

        if let Some(album) = song.metadata.get("album") {
            result.extend(
                album
                    .iter()
                    .map(|item| KeyValue { key: "Album".to_owned(), value: item.to_owned() }),
            );
        }

        let duration = song.duration.as_ref().map(|d| d.as_secs().to_string()).unwrap_or_default();
        if !duration.is_empty() {
            result.push(KeyValue { key: "Duration".to_owned(), value: duration });
        }

        result.extend(
            song.metadata
                .iter()
                .filter(|(key, _)| {
                    !["title", "album", "artist", "duration"].contains(&(*key).as_str())
                })
                .flat_map(|(k, v)| {
                    v.iter().map(|item| KeyValue { key: k.to_owned(), value: item.to_owned() })
                }),
        );

        KeyValues(result)
    }
}
