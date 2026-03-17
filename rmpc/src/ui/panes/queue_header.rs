use std::{cmp::Ordering, collections::HashMap};

use anyhow::Result;
use itertools::Itertools;
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    widgets::{Row, Table},
};
use rmpc_mpd::{commands::Song, mpd_client::MpdCommand, proto_client::ProtoClient};

use crate::{
    config::theme::properties::{Property, PropertyKindOrText, SongProperty},
    ctx::Ctx,
    shared::{
        cmp::StringCompare,
        keys::ActionEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::{
        UiEvent,
        panes::{Pane, queue::QueuePane},
        song_ext::SongExt as _,
    },
};

#[derive(Debug)]
pub struct QueueHeaderPane {
    area: Rect,
    column_widths: Vec<Constraint>,
    column_formats: Vec<Property<SongProperty>>,
    song: Song,
}

impl QueueHeaderPane {
    pub fn new(ctx: &Ctx) -> Self {
        let (column_widths, column_formats) = QueuePane::init(ctx);
        Self { area: Rect::default(), column_widths, column_formats, song: Song::default() }
    }

    pub fn calculate_swaps<T>(evald: &[(u32, T)], ctx: &Ctx) -> Result<Vec<(usize, usize)>> {
        let mut current: Vec<u32> = ctx.queue.iter().map(|s| s.id).collect();
        let mut index: HashMap<u32, usize> =
            current.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let mut swaps = Vec::new();

        for i in 0..current.len() {
            let target_id = evald[i].0;
            if current[i] == target_id {
                continue; // already at the correct position
            }

            let j = *index
                .get(&target_id)
                .ok_or_else(|| anyhow::anyhow!("desired contains an ID not present in current"))?;

            swaps.push((i, j));

            let ai = current[i];
            current.swap(i, j);

            index.insert(ai, j);
            index.insert(target_id, i);
        }

        Ok(swaps)
    }

    pub fn sort_by_column(
        column_formats: &[Property<SongProperty>],
        idx: usize,
        ctx: &Ctx,
    ) -> Result<()> {
        if let Some(format) = column_formats.get(idx) {
            let mut evald = Self::evaluate_content(format, ctx);
            let cmp = StringCompare::builder().fold_case(true).build();
            let is_non_decreasing = evald.is_sorted_by(|(_, a), (_, b)| {
                matches!(cmp.compare(a.as_ref(), b.as_ref()), Ordering::Less | Ordering::Equal)
            });

            if is_non_decreasing {
                evald.sort_by(|(_, a), (_, b)| cmp.compare(a.as_ref(), b.as_ref()).reverse());
            } else {
                evald.sort_by(|(_, a), (_, b)| cmp.compare(a.as_ref(), b.as_ref()));
            }

            let swaps = Self::calculate_swaps(evald.as_slice(), ctx)?;

            ctx.command(move |client| {
                client.send_start_cmd_list()?;
                for swap in swaps {
                    client.send_swap_position(swap.0, swap.1)?;
                }
                client.send_execute_cmd_list()?;
                client.read_ok()?;
                Ok(())
            });
        } else {
            log::error!("Invalid column index for sorting: {idx}");
        }

        Ok(())
    }

    pub fn evaluate_content(format: &Property<SongProperty>, ctx: &Ctx) -> Vec<(u32, String)> {
        match &format.kind {
            PropertyKindOrText::Text(_) => {
                // Do nothing, everything is a constant text
                Vec::new()
            }
            PropertyKindOrText::Sticker(sticker_name) => ctx
                .queue
                .iter()
                .map(|song| {
                    (
                        song.id,
                        ctx.song_stickers(&song.file)
                            .and_then(|s| s.get(sticker_name))
                            .cloned()
                            .unwrap_or_default(),
                    )
                })
                .collect_vec(),
            PropertyKindOrText::Property(_)
            | PropertyKindOrText::Group(_)
            | PropertyKindOrText::Transform(_) => ctx
                .queue
                .iter()
                .map(|song| {
                    (
                        song.id,
                        format
                            .as_string(
                                Some(song),
                                "",
                                ctx.config.theme.multiple_tag_resolution_strategy,
                                ctx,
                            )
                            .unwrap_or_default(),
                    )
                })
                .collect_vec(),
        }
    }
}

impl Pane for QueueHeaderPane {
    fn render(&mut self, frame: &mut Frame, mut area: Rect, ctx: &Ctx) -> Result<()> {
        // Reserve space for the queue scrollbar and padding on the right
        area.width = area.width.saturating_sub(2);
        self.area = area;

        let widths = Layout::horizontal(self.column_widths.as_slice())
            .flex(Flex::Start)
            .spacing(1)
            .split(self.area);

        let header = ctx
            .config
            .theme
            .song_table_format
            .iter()
            .enumerate()
            .map(|(idx, format)| {
                let max_len: usize = widths[idx].width.into();
                self.song
                    .as_line_ellipsized(
                        &format.label,
                        max_len,
                        &ctx.config.theme.symbols,
                        &ctx.config.theme.format_tag_separator,
                        ctx.config.theme.multiple_tag_resolution_strategy,
                        ctx,
                    )
                    .unwrap_or_default()
                    .alignment(format.alignment.into())
            })
            .collect_vec();
        let header_table = Table::new(std::iter::once(Row::new(header)), &self.column_widths);
        frame.render_widget(header_table, self.area);

        Ok(())
    }

    fn handle_action(&mut self, _event: &mut ActionEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        let position = event.into();

        if !self.area.contains(position) {
            return Ok(());
        }

        match event.kind {
            MouseEventKind::LeftClick | MouseEventKind::DoubleClick => {
                if self.area.contains(event.into()) {
                    let widths = Layout::horizontal(self.column_widths.as_slice())
                        .flex(Flex::Start)
                        .spacing(1)
                        .split(self.area);
                    if let Some(header_idx) = widths.iter().position(|w| w.contains(position)) {
                        Self::sort_by_column(self.column_formats.as_slice(), header_idx, ctx)?;
                    }
                    ctx.render()?;
                }
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollDown => {}
            MouseEventKind::ScrollUp => {}
            MouseEventKind::Drag { .. } => {}
        }

        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, _is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::ConfigChanged => {
                let (column_widths, column_formats) = QueuePane::init(ctx);
                self.column_formats = column_formats;
                self.column_widths = column_widths;
            }
            _ => {}
        }
        Ok(())
    }
}
