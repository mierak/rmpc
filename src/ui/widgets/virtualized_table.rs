use itertools::Itertools;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::Style,
    widgets::{Row, StatefulWidget, Table, TableState},
};

use crate::ui::dirstack::DirState;

/// A simple wrapper around ratatui's Table widget which virtaulizes the rows
/// iterator to only materialize the rows necessary for rendering. This is why
/// this table only takes Iterator and not `IntoIterator`.
#[derive(Debug)]
pub struct VirtualizedTable<'a, T: Iterator<Item = Row<'a>>> {
    rows: T,
    column_widths: Vec<Constraint>,
    row_highlight_style: Style,
}

impl<'a, T: Iterator<Item = Row<'a>>> VirtualizedTable<'a, T> {
    pub fn new(rows: T) -> Self {
        Self { rows, column_widths: Vec::new(), row_highlight_style: Style::default() }
    }

    pub fn column_widths<I>(mut self, widths: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Constraint>,
    {
        self.column_widths = widths.into_iter().map(Into::into).collect_vec();
        self
    }

    pub fn row_highlight_style(mut self, style: Style) -> Self {
        self.row_highlight_style = style;
        self
    }
}

impl<'a, T: Iterator<Item = Row<'a>>> StatefulWidget for VirtualizedTable<'a, T> {
    type State = DirState<TableState>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
        let Some(viewport_len) = state.viewport_len() else {
            return;
        };

        // Save original state and remove offset because ratatui's table will think that
        // we are rendering from item 0 to viewport_len, the rest will be ignored
        let original_offset = state.offset();
        let original_selected = state.inner.selected();
        *state.inner.offset_mut() = 0;
        state.select(original_selected.map(|v| v.saturating_sub(original_offset)), 0);

        let actual_rows = self.rows.skip(original_offset).take(viewport_len);
        let table = Table::new(actual_rows, self.column_widths)
            .row_highlight_style(self.row_highlight_style);

        StatefulWidget::render(table, area, buf, state.as_render_state_ref());

        // Restore the original state
        *state.inner.offset_mut() = original_offset;
        state.select(original_selected, 0);
    }
}
