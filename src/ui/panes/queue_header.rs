use std::{cmp::Ordering, collections::HashMap};

use anyhow::Result;
use itertools::Itertools;
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    widgets::{Row, Table},
};

use crate::{
    config::theme::properties::{Property, PropertyKindOrText, SongProperty},
    ctx::Ctx,
    mpd::{commands::Song, mpd_client::MpdCommand, proto_client::ProtoClient},
    shared::{
        cmp::StringCompare,
        keys::ActionEvent,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::{
        UiEvent,
        panes::{Pane, queue::QueuePane},
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

    fn calculate_swaps<T: AsRef<str>>(
        mut desired: Vec<(u32, T)>,
        ctx: &Ctx,
    ) -> Result<Vec<(usize, usize)>> {
        let cmp = StringCompare::builder().fold_case(true).build();
        let is_non_decreasing = desired.is_sorted_by(|(_, a), (_, b)| {
            matches!(cmp.compare(a.as_ref(), b.as_ref()), Ordering::Less | Ordering::Equal)
        });

        if is_non_decreasing {
            desired.sort_by(|(_, a), (_, b)| cmp.compare(a.as_ref(), b.as_ref()).reverse());
        } else {
            desired.sort_by(|(_, a), (_, b)| cmp.compare(a.as_ref(), b.as_ref()));
        }

        let mut current: Vec<u32> = ctx.queue.iter().map(|s| s.id).collect();
        let mut index: HashMap<u32, usize> =
            current.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let mut swaps = Vec::new();

        for i in 0..current.len() {
            let target_id = desired[i].0;
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
        let swaps = match column_formats.get(idx).as_ref().map(|v| &v.kind) {
            Some(PropertyKindOrText::Text(_)) => {
                // Do nothing, everything is a constant text
                Vec::new()
            }
            Some(PropertyKindOrText::Sticker(sticker_name)) => {
                let evald = ctx
                    .queue
                    .iter()
                    .map(|song| {
                        (
                            song.id,
                            ctx.song_stickers(&song.file)
                                .and_then(|s| s.get(sticker_name))
                                .map(|s| s.as_str())
                                .unwrap_or_default(),
                        )
                    })
                    .collect_vec();

                Self::calculate_swaps(evald, ctx)?
            }
            Some(PropertyKindOrText::Property(_))
            | Some(PropertyKindOrText::Group(_))
            | Some(PropertyKindOrText::Transform(_)) => {
                let evald = ctx
                    .queue
                    .iter()
                    .map(|song| {
                        (
                            song.id,
                            column_formats[idx]
                                .as_string(
                                    Some(song),
                                    "",
                                    ctx.config.theme.multiple_tag_resolution_strategy,
                                    ctx,
                                )
                                .unwrap_or_default(),
                        )
                    })
                    .collect_vec();

                Self::calculate_swaps(evald, ctx)?
            }
            None => {
                // Should not really ever happen. But no reason to handle this as a hard error.
                log::warn!("Tried to sort by non-existing column index {idx}");
                Vec::new()
            }
        };

        ctx.command(move |client| {
            client.send_start_cmd_list()?;
            for swap in swaps {
                client.send_swap_position(swap.0, swap.1)?;
            }
            client.send_execute_cmd_list()?;
            client.read_ok()?;
            Ok(())
        });

        Ok(())
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
