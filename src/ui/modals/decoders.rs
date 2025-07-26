use anyhow::Result;
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
    ctx::Ctx,
    mpd::commands::Decoder,
    shared::{
        ext::iter::IntoZipLongest2,
        id::{self, Id},
        key_event::KeyEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::dirstack::DirState,
};

#[derive(Debug)]
pub struct DecodersModal {
    id: Id,
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
        let mut result = Self {
            id: id::new(),
            decoders,
            scrolling_state: DirState::default(),
            table_area: Rect::default(),
        };
        result.scrolling_state.set_content_len(Some(result.decoders.len()));
        result.scrolling_state.first();

        result
    }

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
    fn id(&self) -> Id {
        self.id
    }

    fn render(&mut self, frame: &mut Frame, ctx: &mut Ctx) -> Result<()> {
        let popup_area = frame.area().centered(80, 80);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = ctx.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(ctx.config.as_border_style())
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

        self.scrolling_state.set_content_and_viewport_len(rows.len(), table_area.height.into());

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
            Block::default().borders(Borders::BOTTOM).border_style(ctx.config.as_border_style()),
        );
        let table = Table::new(rows, [
            Constraint::Percentage(name_col_width),
            Constraint::Percentage(mime_col_width),
            Constraint::Percentage(suffix_col_width),
        ])
        .column_spacing(1)
        .style(ctx.config.as_text_style())
        .row_highlight_style(ctx.config.theme.current_item_style);

        self.table_area = table_area;

        frame.render_widget(block, popup_area);
        frame.render_widget(header_table, header_area);
        frame.render_stateful_widget(table, table_area, self.scrolling_state.as_render_state_ref());
        if let Some(scrollbar) = ctx.config.as_styled_scrollbar() {
            frame.render_stateful_widget(
                scrollbar,
                popup_area.inner(Margin { horizontal: 0, vertical: 1 }),
                self.scrolling_state.as_scrollbar_state_ref(),
            );
        }

        return Ok(());
    }

    fn handle_key(&mut self, key: &mut KeyEvent, ctx: &mut Ctx) -> Result<()> {
        if let Some(action) = key.as_common_action(ctx) {
            match action {
                CommonAction::DownHalf => {
                    self.scrolling_state.next_half_viewport(ctx.config.scrolloff);

                    ctx.render()?;
                }
                CommonAction::UpHalf => {
                    self.scrolling_state.prev_half_viewport(ctx.config.scrolloff);

                    ctx.render()?;
                }
                CommonAction::Up => {
                    self.scrolling_state.prev(ctx.config.scrolloff, ctx.config.wrap_navigation);

                    ctx.render()?;
                }
                CommonAction::Down => {
                    self.scrolling_state.next(ctx.config.scrolloff, ctx.config.wrap_navigation);

                    ctx.render()?;
                }
                CommonAction::Bottom => {
                    self.scrolling_state.last();

                    ctx.render()?;
                }
                CommonAction::Top => {
                    self.scrolling_state.first();

                    ctx.render()?;
                }
                CommonAction::Confirm => {}
                CommonAction::Close => {
                    self.hide(ctx)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &mut Ctx) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick if self.table_area.contains(event.into()) => {
                let y: usize = event.y.saturating_sub(self.table_area.y).into();
                if let Some(idx) = self.scrolling_state.get_at_rendered_row(y) {
                    self.scrolling_state.select(Some(idx), ctx.config.scrolloff);
                    ctx.render()?;
                }
            }
            MouseEventKind::DoubleClick => {}
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollDown if self.table_area.contains(event.into()) => {
                self.scrolling_state.scroll_down(1, ctx.config.scrolloff);
                ctx.render()?;
            }
            MouseEventKind::ScrollUp if self.table_area.contains(event.into()) => {
                self.scrolling_state.scroll_up(1, ctx.config.scrolloff);
                ctx.render()?;
            }
            MouseEventKind::LeftClick => {}
            MouseEventKind::ScrollDown => {}
            MouseEventKind::ScrollUp => {}
            MouseEventKind::Drag { drag_start_position: _ } => {}
        }

        Ok(())
    }
}
