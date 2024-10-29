use ratatui::{
    layout::{Constraint, Margin},
    style::Style,
    symbols::border,
    widgets::{Block, Borders, Cell, Clear, Row, Table, TableState},
};

use crate::{
    config::keys::CommonAction,
    context::AppContext,
    mpd::{commands::Output, mpd_client::MpdClient},
    ui::{dirstack::DirState, KeyHandleResultInternal},
};

use super::{Modal, RectExt};

#[derive(Debug)]
pub struct OutputsModal {
    scrolling_state: DirState<TableState>,
    outputs: Vec<Output>,
}

impl OutputsModal {
    pub fn new(outputs: Vec<Output>) -> Self {
        let mut result = Self {
            outputs,
            scrolling_state: DirState::default(),
        };
        result.scrolling_state.set_content_len(Some(result.outputs.len()));
        result.scrolling_state.first();

        result
    }
}

impl Modal for OutputsModal {
    fn render(&mut self, frame: &mut ratatui::Frame, app: &mut crate::context::AppContext) -> anyhow::Result<()> {
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
            .title("Keybinds");

        let table_area = popup_area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });

        let rows = self.outputs.iter().map(|output| {
            Row::new([
                Cell::from(output.id.to_string()),
                Cell::from(output.name.clone()),
                Cell::from(if output.enabled { "yes" } else { "no" }),
            ])
        });

        self.scrolling_state.set_viewport_len(Some(table_area.height.into()));

        let table = Table::new(
            rows,
            [
                Constraint::Length(3),
                Constraint::Percentage(100),
                Constraint::Length(10),
            ],
        )
        .column_spacing(0)
        .style(app.config.as_text_style())
        .header(Row::new(["Id", "Name", "Enabled"]))
        .row_highlight_style(app.config.theme.current_item_style);

        frame.render_widget(block, popup_area);
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

        Ok(())
    }

    fn handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        client: &mut crate::mpd::client::Client<'_>,
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
                    self.scrolling_state
                        .prev(context.config.scrolloff, context.config.wrap_navigation);
                    Ok(KeyHandleResultInternal::RenderRequested)
                }
                CommonAction::Down => {
                    self.scrolling_state
                        .next(context.config.scrolloff, context.config.wrap_navigation);
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
                CommonAction::Confirm => {
                    let Some(idx) = self.scrolling_state.get_selected() else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };
                    let Some(output) = self.outputs.get(idx) else {
                        return Ok(KeyHandleResultInternal::SkipRender);
                    };
                    client.toggle_output(output.id)?;
                    self.outputs = client.outputs()?.0;

                    if idx >= self.outputs.len() {
                        self.scrolling_state.last();
                    }

                    Ok(KeyHandleResultInternal::SkipRender)
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
