use std::{borrow::Cow, collections::HashMap, fmt::Display};

use crate::{
    config::keys::{CommonAction, Key, ToDescription},
    context::AppContext,
};
use anyhow::bail;
use ratatui::{
    layout::{Constraint, Layout, Margin},
    style::Style,
    symbols::border,
    text::Line,
    widgets::{Block, Borders, Cell, Clear, Row, Table, TableState},
    Frame,
};

use crate::ui::{utils::dirstack::DirState, KeyHandleResultInternal};

use super::{Modal, RectExt};

#[derive(Debug)]
pub struct KeybindsModal<'a> {
    scrolling_state: DirState<TableState>,
    rows: Vec<Row<'a>>,
}

fn add_binds<'a, V: Display + ToDescription>(
    result: &mut Vec<Row<'a>>,
    binds: &HashMap<Key, V>,
    name: &'a str,
    header_style: Style,
    add_empty_line: bool,
) {
    if !binds.is_empty() {
        result.push(
            Row::new(vec![
                Line::raw(Cow::<str>::Borrowed(" ")).patch_style(header_style),
                Line::raw(Cow::<str>::Borrowed(name))
                    .patch_style(header_style)
                    .centered(),
                Line::raw(Cow::<str>::Borrowed(" ")).patch_style(header_style),
            ])
            .top_margin(u16::from(add_empty_line)),
        );
    }
    for (key, action) in binds {
        result.push(Row::new(vec![
            key.to_string(),
            action.to_string(),
            action.to_description().to_string(),
        ]));
    }
}

impl KeybindsModal<'_> {
    pub fn new(app: &mut crate::context::AppContext) -> Self {
        let keybinds = &app.config.keybinds;
        let header_style = app.config.theme.current_item_style;

        let mut rows = Vec::new();
        add_binds(&mut rows, &keybinds.global, "Global", header_style, false);
        add_binds(&mut rows, &keybinds.navigation, "Navigation", header_style, true);
        add_binds(&mut rows, &keybinds.albums, "Albums", header_style, true);
        add_binds(&mut rows, &keybinds.artists, "Artists", header_style, true);
        add_binds(&mut rows, &keybinds.directories, "Directories", header_style, true);
        add_binds(&mut rows, &keybinds.playlists, "Playlists", header_style, true);
        add_binds(&mut rows, &keybinds.search, "Search", header_style, true);
        add_binds(&mut rows, &keybinds.queue, "Queue", header_style, true);

        let mut scrolling_state = DirState::default();
        if !rows.is_empty() {
            scrolling_state.select(Some(0), 0);
        }
        Self { scrolling_state, rows }
    }
}

impl Modal for KeybindsModal<'_> {
    fn render(&mut self, frame: &mut Frame, app: &mut crate::context::AppContext) -> anyhow::Result<()> {
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

        let [header_area, table_area] =
            *Layout::vertical([Constraint::Length(2), Constraint::Percentage(100)]).split(popup_area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }))
        else {
            bail!("Failed to split help modal area");
        };

        self.scrolling_state.set_content_len(Some(self.rows.len()));
        self.scrolling_state.set_viewport_len(Some(table_area.height.into()));

        let header_table = Table::new(
            vec![Row::new([
                Cell::from("Key"),
                Cell::from("Action"),
                Cell::from("Description"),
            ])],
            [
                Constraint::Percentage(20),
                Constraint::Percentage(30),
                Constraint::Percentage(50),
            ],
        )
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(app.config.as_border_style()),
        );
        let table = Table::new(
            self.rows.clone(),
            [
                Constraint::Percentage(20),
                Constraint::Percentage(30),
                Constraint::Percentage(50),
            ],
        )
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
                    self.scrolling_state.prev(context.config.scrolloff);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    self.scrolling_state.next(context.config.scrolloff);
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
