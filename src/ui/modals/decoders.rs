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
use crate::mpd::commands::Decoder;
use crate::shared::ext::iter::IntoZipLongest2;
use crate::shared::key_event::KeyEvent;
use crate::shared::macros::pop_modal;
use crate::shared::mouse_event::{MouseEvent, MouseEventKind};
use crate::ui::dirstack::DirState;

#[derive(Debug)]
pub struct DecodersModal {
    scrolling_state: DirState<TableState>,
    table_area: Rect,
    decoders: Vec<(String, String, String)>,
}

impl DecodersModal {
    pub fn new(decoders: Vec<Decoder>) -> Self {
        let decoders = decoders
            .into_iter()
            .map(|decoder| {
                let name = decoder.name.clone();
                let mime = decoder.mime_types.join(", ");
                let suffixes = decoder.suffixes.join(", ");
                (name, mime, suffixes)
            })
            .collect();
        let mut result =
            Self { decoders, scrolling_state: DirState::default(), table_area: Rect::default() };
        result.scrolling_state.set_content_len(Some(result.decoders.len()));
        result.scrolling_state.first();

        result
    }

    #[allow(clippy::cast_possible_truncation)]
    fn row<'a>(
        name: &'a str,
        name_width: u16,
        mime: &'a str,
        mime_width: u16,
        suffixes: &'a str,
        suffixes_width: u16,
    ) -> impl Iterator<Item = Row<'a>> {
        let name = textwrap::wrap(name, name_width as usize);
        let mime = textwrap::wrap(mime, mime_width as usize);
        let suffixes = textwrap::wrap(suffixes, suffixes_width as usize);

        name.into_iter().zip_longest2(mime.into_iter(), suffixes.into_iter()).map(
            |(name, mime, suffix)| {
                Row::new([
                    Cell::from(Text::from(name.unwrap_or_default())),
                    Cell::from(Text::from(mime.unwrap_or_default())),
                    Cell::from(Text::from(suffix.unwrap_or_default())),
                ])
            },
        )
    }
}

impl Modal for DecodersModal {
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
            .title("Decoder plugins");

        let (name_col_width, mime_col_width, suffix_col_width) = (10, 45, 45);
        let margin = Margin { horizontal: 1, vertical: 0 };
        let [header_area, table_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Percentage(100)])
                .areas(block.inner(popup_area));
        let header_area = header_area.inner(margin);
        let table_area = table_area.inner(margin);

        let [name_area, mut mime_area, suffix_area] = Layout::horizontal([
            Constraint::Percentage(name_col_width),
            Constraint::Percentage(mime_col_width),
            Constraint::Percentage(suffix_col_width),
        ])
        .areas(table_area);
        mime_area.width = mime_area.width.saturating_sub(1); // account for the column spacing

        let Self { decoders, .. } = self;
        let rows = decoders
            .iter()
            .flat_map(|(name, mime, suffixes)| {
                DecodersModal::row(
                    name,
                    name_area.width,
                    mime,
                    mime_area.width,
                    suffixes,
                    suffix_area.width,
                )
            })
            .collect_vec();

        self.scrolling_state.set_content_len(Some(rows.len()));
        self.scrolling_state.set_viewport_len(Some(table_area.height.into()));

        let header_table = Table::new(
            vec![Row::new([
                Cell::from("Plugin"),
                Cell::from("MIME types"),
                Cell::from("Suffixes"),
            ])],
            [
                Constraint::Percentage(name_col_width),
                Constraint::Percentage(mime_col_width),
                Constraint::Percentage(suffix_col_width),
            ],
        )
        .column_spacing(1)
        .block(
            Block::default().borders(Borders::BOTTOM).border_style(app.config.as_border_style()),
        );
        let table = Table::new(rows, [
            Constraint::Percentage(name_col_width),
            Constraint::Percentage(mime_col_width),
            Constraint::Percentage(suffix_col_width),
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
                CommonAction::Confirm => {}
                CommonAction::Close => {
                    pop_modal!(context);
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, context: &mut AppContext) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick if self.table_area.contains(event.into()) => {
                let y: usize = event.y.saturating_sub(self.table_area.y).into();
                if let Some(idx) = self.scrolling_state.get_at_rendered_row(y) {
                    self.scrolling_state.select(Some(idx), context.config.scrolloff);
                    context.render()?;
                }
            }
            MouseEventKind::DoubleClick => {}
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollDown if self.table_area.contains(event.into()) => {
                self.scrolling_state.next(context.config.scrolloff, false);
                context.render()?;
            }
            MouseEventKind::ScrollUp if self.table_area.contains(event.into()) => {
                self.scrolling_state.prev(context.config.scrolloff, false);
                context.render()?;
            }
            MouseEventKind::LeftClick => {}
            MouseEventKind::ScrollDown => {}
            MouseEventKind::ScrollUp => {}
        }

        Ok(())
    }
}
