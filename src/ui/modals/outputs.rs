use anyhow::Result;
use ratatui::{
    layout::{Constraint, Margin, Rect},
    style::Style,
    symbols::border,
    widgets::{Block, Borders, Cell, Clear, Row, Table, TableState},
};

use super::{Modal, RectExt};
use crate::{
    MpdQueryResult,
    config::keys::CommonAction,
    context::AppContext,
    mpd::{commands::Output, mpd_client::MpdClient},
    shared::{
        key_event::KeyEvent,
        macros::pop_modal,
        mouse_event::{MouseEvent, MouseEventKind},
    },
    ui::dirstack::DirState,
};

#[derive(Debug)]
pub struct OutputsModal {
    scrolling_state: DirState<TableState>,
    outputs_table_area: Rect,
    outputs: Vec<Output>,
}

impl OutputsModal {
    pub fn new(outputs: Vec<Output>) -> Self {
        let mut result = Self {
            outputs,
            scrolling_state: DirState::default(),
            outputs_table_area: Rect::default(),
        };
        result.scrolling_state.set_content_len(Some(result.outputs.len()));
        result.scrolling_state.first();

        result
    }

    pub fn toggle_selected_output(&mut self, context: &AppContext) {
        let Some(idx) = self.scrolling_state.get_selected() else {
            return;
        };
        let Some(output) = self.outputs.get(idx) else {
            return;
        };

        let id = output.id;
        context.query().id("refresh_outputs").query(move |client| {
            client.toggle_output(id)?;
            Ok(MpdQueryResult::Outputs(client.outputs()?.0))
        });
    }
}

impl Modal for OutputsModal {
    fn render(&mut self, frame: &mut ratatui::Frame, app: &mut AppContext) -> anyhow::Result<()> {
        let popup_area = frame.area().centered_exact(60, 10);
        frame.render_widget(Clear, popup_area);
        if let Some(bg_color) = app.config.theme.modal_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(bg_color)), popup_area);
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(app.config.as_border_style())
            .title_alignment(ratatui::prelude::Alignment::Center)
            .title("Outputs");

        let table_area = popup_area.inner(Margin { horizontal: 1, vertical: 1 });

        let rows = self.outputs.iter().map(|output| {
            Row::new([
                Cell::from(output.id.to_string()),
                Cell::from(output.name.clone()),
                Cell::from(if output.enabled { "yes" } else { "no" }),
            ])
        });

        self.scrolling_state.set_viewport_len(Some(table_area.height.into()));

        let table = Table::new(rows, [
            Constraint::Length(3),
            Constraint::Percentage(100),
            Constraint::Length(10),
        ])
        .column_spacing(0)
        .style(app.config.as_text_style())
        .header(Row::new(["Id", "Name", "Enabled"]))
        .row_highlight_style(app.config.theme.current_item_style);

        let table_area = table_area.inner(Margin { horizontal: 1, vertical: 0 });
        self.outputs_table_area = table_area;

        frame.render_widget(block, popup_area);
        frame.render_stateful_widget(table, table_area, self.scrolling_state.as_render_state_ref());
        if let Some(scrollbar) = app.config.as_styled_scrollbar() {
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
        context: &AppContext,
    ) -> Result<()> {
        match (id, data) {
            ("refresh_outputs", MpdQueryResult::Outputs(outputs)) => {
                self.outputs = std::mem::take(outputs);
                context.render()?;
            }
            _ => {}
        }
        Ok(())
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
                CommonAction::Confirm => {
                    self.toggle_selected_output(context);
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
        match event.kind {
            MouseEventKind::LeftClick if self.outputs_table_area.contains(event.into()) => {
                let y: usize = event.y.saturating_sub(self.outputs_table_area.y).into();
                let y = y.saturating_sub(1); // Subtract one to account for table header
                if let Some(idx) = self.scrolling_state.get_at_rendered_row(y) {
                    self.scrolling_state.select(Some(idx), context.config.scrolloff);
                    context.render()?;
                }
            }
            MouseEventKind::DoubleClick if self.outputs_table_area.contains(event.into()) => {
                self.toggle_selected_output(context);
                context.render()?;
            }
            MouseEventKind::MiddleClick => {}
            MouseEventKind::RightClick => {}
            MouseEventKind::ScrollDown if self.outputs_table_area.contains(event.into()) => {
                self.scrolling_state.next(context.config.scrolloff, false);
                context.render()?;
            }
            MouseEventKind::ScrollUp if self.outputs_table_area.contains(event.into()) => {
                self.scrolling_state.prev(context.config.scrolloff, false);
                context.render()?;
            }
            MouseEventKind::LeftClick => {}
            MouseEventKind::DoubleClick => {}
            MouseEventKind::ScrollDown => {}
            MouseEventKind::ScrollUp => {}
        }

        Ok(())
    }
}
