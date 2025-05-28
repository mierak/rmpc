use std::{borrow::Cow, collections::HashMap, fmt::Display};

use anyhow::Result;
use itertools::Itertools;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::Style,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Borders, Cell, Clear, Row, Table, TableState},
};

use super::{Modal, RectExt};
use crate::{
    config::keys::{CommonAction, ToDescription},
    context::AppContext,
    shared::{
        ext::iter::IntoZipLongest2,
        key_event::KeyEvent,
        macros::pop_modal,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::dirstack::DirState,
};

#[derive(Debug)]
pub struct KeybindsModal {
    scrolling_state: DirState<TableState>,
    table_area: Rect,
}

trait KeybindsExt {
    fn to_str(&self) -> impl Iterator<Item = (String, String, Cow<'static, str>)>;
}

impl<K: Display, V: Display + ToDescription> KeybindsExt for HashMap<K, V> {
    fn to_str(&self) -> impl Iterator<Item = (String, String, Cow<'static, str>)> {
        self.iter().map(|(key, value)| (key.to_string(), value.to_string(), value.to_description()))
    }
}

impl KeybindsModal {
    pub fn new(_app: &mut AppContext) -> Self {
        let mut scrolling_state = DirState::default();
        scrolling_state.select(Some(0), 0);
        Self { scrolling_state, table_area: Rect::default() }
    }
}
fn row_header<'a>(
    keys: &'a [(String, String, Cow<'a, str>)],
    name: &'a str,
    header_style: Style,
) -> Option<Row<'a>> {
    if keys.is_empty() {
        None
    } else {
        Some(Row::new(vec![
            Line::raw(Cow::<str>::Borrowed(" ")).patch_style(header_style),
            Line::raw(Cow::<str>::Borrowed(name)).patch_style(header_style).centered(),
            Line::raw(Cow::<str>::Borrowed(" ")).patch_style(header_style),
        ]))
    }
}

#[allow(clippy::cast_possible_truncation)]
fn row<'a>(
    keys: &'a [(String, String, Cow<'a, str>)],
    key_width: u16,
    action_width: u16,
    description_width: u16,
) -> impl Iterator<Item = Row<'a>> {
    keys.iter().flat_map(move |(key, action, description)| {
        let key = textwrap::wrap(key, key_width as usize);
        let action = textwrap::wrap(action, action_width as usize);
        let description = textwrap::wrap(description, description_width as usize);

        key.into_iter().zip_longest2(action.into_iter(), description.into_iter()).map(
            |(key, action, description)| {
                Row::new([
                    Cell::from(Text::from(key.unwrap_or_default())),
                    Cell::from(Text::from(action.unwrap_or_default())),
                    Cell::from(Text::from(description.unwrap_or_default())),
                ])
            },
        )
    })
}

impl Modal for KeybindsModal {
    fn render(&mut self, frame: &mut Frame, app: &mut AppContext) -> Result<()> {
        let popup_area = frame.area().centered(90, 90);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = app.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(app.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center)
            .title("Keybinds");

        let margin = Margin { horizontal: 1, vertical: 0 };
        let [header_area, table_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Percentage(100)])
                .areas(block.inner(popup_area));
        let header_area = header_area.inner(margin);
        let table_area = table_area.inner(margin);

        let (key_width, action_width, desc_width) = (20, 30, 50);
        let constraints = [
            Constraint::Percentage(key_width),
            Constraint::Percentage(action_width),
            Constraint::Percentage(desc_width),
        ];
        let [key_area, mut action_area, mut desc_area] =
            Layout::horizontal(constraints).areas(table_area);
        action_area.width = action_area.width.saturating_sub(1); // account for the column spacing
        desc_area.width = desc_area.width.saturating_sub(2); // account for the column spacing

        let keybinds = &app.config.keybinds;
        let header_style = app.config.theme.current_item_style;

        let mut global: Vec<_> = keybinds.global.to_str().collect();
        let mut navigation: Vec<_> = keybinds.navigation.to_str().collect();
        let mut albums: Vec<_> = keybinds.albums.to_str().collect();
        let mut artists: Vec<_> = keybinds.artists.to_str().collect();
        let mut directories: Vec<_> = keybinds.directories.to_str().collect();
        let mut playlists: Vec<_> = keybinds.playlists.to_str().collect();
        let mut search: Vec<_> = keybinds.search.to_str().collect();
        let mut queue: Vec<_> = keybinds.queue.to_str().collect();

        global.sort_by_key(|(_, action, _)| action.to_lowercase());
        navigation.sort_by_key(|(_, action, _)| action.to_lowercase());
        albums.sort_by_key(|(_, action, _)| action.to_lowercase());
        artists.sort_by_key(|(_, action, _)| action.to_lowercase());
        directories.sort_by_key(|(_, action, _)| action.to_lowercase());
        playlists.sort_by_key(|(_, action, _)| action.to_lowercase());
        search.sort_by_key(|(_, action, _)| action.to_lowercase());
        queue.sort_by_key(|(_, action, _)| action.to_lowercase());

        let rows = row_header(&global, "Global", header_style)
            .into_iter()
            .chain(row(&global, key_area.width, action_area.width, desc_area.width))
            .chain(row_header(&navigation, "Navigation", header_style))
            .chain(row(&navigation, key_area.width, action_area.width, desc_area.width))
            .chain(row_header(&albums, "Albums", header_style))
            .chain(row(&albums, key_area.width, action_area.width, desc_area.width))
            .chain(row_header(&artists, "Artists", header_style))
            .chain(row(&artists, key_area.width, action_area.width, desc_area.width))
            .chain(row_header(&directories, "Directories", header_style))
            .chain(row(&directories, key_area.width, action_area.width, desc_area.width))
            .chain(row_header(&playlists, "Playlists", header_style))
            .chain(row(&playlists, key_area.width, action_area.width, desc_area.width))
            .chain(row_header(&search, "Search", header_style))
            .chain(row(&search, key_area.width, action_area.width, desc_area.width))
            .chain(row_header(&queue, "Queue", header_style))
            .chain(row(&queue, key_area.width, action_area.width, desc_area.width))
            .collect_vec();

        self.scrolling_state.set_content_len(Some(rows.len()));
        self.scrolling_state.set_viewport_len(Some(table_area.height.into()));

        let header_table = Table::new(
            vec![Row::new([Cell::from("Key"), Cell::from("Action"), Cell::from("Description")])],
            constraints,
        )
        .column_spacing(1)
        .block(
            Block::default().borders(Borders::BOTTOM).border_style(app.config.as_border_style()),
        );

        let table = Table::new(rows, constraints)
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
