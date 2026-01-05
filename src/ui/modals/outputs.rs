use anyhow::Result;
use ratatui::{
    layout::{Constraint, Margin, Rect},
    macros::constraint,
    style::Style,
    symbols::border,
    widgets::{Block, Borders, Cell, Clear, Row, Table, TableState},
};

use super::Modal;
use crate::{
    MpdQueryResult,
    config::keys::CommonAction,
    ctx::Ctx,
    mpd::mpd_client::MpdClient,
    shared::{
        id::{self, Id},
        keys::ActionEvent,
        mouse_event::{MouseEvent, MouseEventKind},
        mpd_client_ext::{MpdClientExt, PartitionedOutput, PartitionedOutputKind},
    },
    ui::{UiEvent, dirstack::DirState},
};

#[derive(Debug)]
pub struct OutputsModal {
    id: Id,
    scrolling_state: DirState<TableState>,
    outputs_table_area: Rect,
    outputs: Vec<PartitionedOutput>,
}

impl OutputsModal {
    pub fn new(outputs: Vec<PartitionedOutput>) -> Self {
        let len = outputs.len();
        let mut result = Self {
            id: id::new(),
            outputs,
            scrolling_state: DirState::default(),
            outputs_table_area: Rect::default(),
        };
        if len > 0 {
            result.scrolling_state.select(Some(0), 0);
        }

        result
    }

    pub fn toggle_selected_output(&mut self, ctx: &Ctx) {
        let Some(idx) = self.scrolling_state.get_selected() else {
            return;
        };
        let Some(output) = self.outputs.get(idx) else {
            return;
        };

        let id = output.id;
        let name = output.name.clone();
        let kind = output.kind;
        let current_partition = ctx.status.partition.clone();
        ctx.query().id("refresh_outputs").query(move |client| {
            match kind {
                PartitionedOutputKind::OtherPartition => {
                    client.move_output(&name)?;
                    let new_outputs = client.outputs()?.0;
                    if let Some(output) = new_outputs.iter().find(|output| output.name == name) {
                        client.enable_output(output.id)?;
                    }
                }
                PartitionedOutputKind::CurrentPartition => {
                    client.toggle_output(id)?;
                }
            }

            Ok(MpdQueryResult::Outputs(client.list_partitioned_outputs(&current_partition)?))
        });
    }

    fn refresh_outputs(&mut self, ctx: &Ctx) {
        let current_partition = ctx.status.partition.clone();
        ctx.query().id("refresh_outputs").replace_id("refresh_outputs").query(move |client| {
            let outputs = client.list_partitioned_outputs(&current_partition)?;
            Ok(MpdQueryResult::Outputs(outputs))
        });
    }
}

impl Modal for OutputsModal {
    fn id(&self) -> Id {
        self.id
    }

    fn render(&mut self, frame: &mut ratatui::Frame, ctx: &mut Ctx) -> anyhow::Result<()> {
        let popup_area = frame.area().centered(constraint!(==70), constraint!(==10));
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = ctx.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(ctx.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center)
            .title("Outputs");

        let table_area = popup_area.inner(Margin { horizontal: 1, vertical: 1 });

        let rows = self.outputs.iter().map(|output| match output.kind {
            PartitionedOutputKind::OtherPartition => Row::new([
                Cell::new(output.name.as_str()),
                Cell::new("-"),
                Cell::new("no"),
                Cell::new("other"),
            ]),
            PartitionedOutputKind::CurrentPartition => Row::new([
                Cell::new(output.name.as_str()),
                Cell::new(output.plugin.as_str()),
                Cell::new(if output.enabled { "yes" } else { "no" }),
                Cell::new("current"),
            ]),
        });

        self.scrolling_state
            .set_content_and_viewport_len(self.outputs.len(), table_area.height.into());

        let table = Table::new(rows, [
            Constraint::Percentage(80),
            Constraint::Percentage(20),
            Constraint::Length(10),
            Constraint::Length(9),
        ])
        .column_spacing(0)
        .style(ctx.config.as_text_style())
        .header(Row::new(["Name", "Plugin", "Enabled", "Partition"]))
        .row_highlight_style(ctx.config.theme.current_item_style);

        let table_area = table_area.inner(Margin { horizontal: 1, vertical: 0 });
        self.outputs_table_area = table_area;

        frame.render_widget(block, popup_area);
        frame.render_stateful_widget(table, table_area, self.scrolling_state.as_render_state_ref());
        if let Some(scrollbar) = ctx.config.as_styled_scrollbar() {
            frame.render_stateful_widget(
                scrollbar,
                popup_area.inner(Margin { horizontal: 0, vertical: 1 }),
                self.scrolling_state.as_scrollbar_state_ref(),
            );
        }

        Ok(())
    }

    fn on_query_finished(
        &mut self,
        id: &'static str,
        data: &mut MpdQueryResult,
        ctx: &Ctx,
    ) -> Result<()> {
        match (id, data) {
            ("refresh_outputs", MpdQueryResult::Outputs(outputs)) => {
                self.outputs = std::mem::take(outputs);
                ctx.render()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::Output => self.refresh_outputs(ctx),
            _ => {}
        }
        Ok(())
    }

    fn handle_key(&mut self, key: &mut ActionEvent, ctx: &mut Ctx) -> Result<()> {
        if let Some(action) = key.claim_common() {
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
                CommonAction::Confirm => {
                    self.toggle_selected_output(ctx);
                }
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
            MouseEventKind::LeftClick if self.outputs_table_area.contains(event.into()) => {
                let y: usize = event.y.saturating_sub(self.outputs_table_area.y).into();
                let y = y.saturating_sub(1); // Subtract one to account for table header
                if let Some(idx) = self.scrolling_state.get_at_rendered_row(y) {
                    self.scrolling_state.select(Some(idx), ctx.config.scrolloff);
                    ctx.render()?;
                }
            }
            MouseEventKind::DoubleClick if self.outputs_table_area.contains(event.into()) => {
                self.toggle_selected_output(ctx);
                ctx.render()?;
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollDown if self.outputs_table_area.contains(event.into()) => {
                self.scrolling_state.scroll_down(ctx.config.scroll_amount, ctx.config.scrolloff);
                ctx.render()?;
            }
            MouseEventKind::ScrollUp if self.outputs_table_area.contains(event.into()) => {
                self.scrolling_state.scroll_up(ctx.config.scroll_amount, ctx.config.scrolloff);
                ctx.render()?;
            }
            MouseEventKind::LeftClick => {}
            MouseEventKind::DoubleClick => {}
            MouseEventKind::ScrollDown => {}
            MouseEventKind::ScrollUp => {}
            MouseEventKind::Drag { drag_start_position: _ } => {}
        }

        Ok(())
    }
}
