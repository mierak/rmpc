use itertools::Itertools;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::Style,
    widgets::{Row, StatefulWidget, Table, TableState},
};

use crate::ui::dirstack::DirState;

/// A simple wrapper around ratatui's Table widget which virtualizes the rows
/// iterator to only materialize the rows necessary for rendering. This is why
/// this table only takes Iterator and not `IntoIterator`.
#[derive(Debug)]
pub struct VirtualizedTable<'a, 'song, T, F>
where
    F: Fn(usize, &'song T) -> Row<'a>,
{
    items: &'song [T],
    column_widths: Vec<Constraint>,
    row_highlight_style: Style,
    map_fn: Option<F>,
}

impl<'a, 'song, T, F> VirtualizedTable<'a, 'song, T, F>
where
    F: Fn(usize, &'song T) -> Row<'a>,
{
    pub fn new(items: &'song [T]) -> Self {
        Self {
            items,
            column_widths: Vec::new(),
            row_highlight_style: Style::default(),
            map_fn: None,
        }
    }

    pub fn map_fn(mut self, f: F) -> Self {
        self.map_fn = Some(f);
        self
    }

    pub fn column_widths<W>(mut self, widths: W) -> Self
    where
        W: IntoIterator,
        W::Item: Into<Constraint>,
    {
        self.column_widths = widths.into_iter().map(Into::into).collect_vec();
        self
    }

    pub fn row_highlight_style(mut self, style: Style) -> Self {
        self.row_highlight_style = style;
        self
    }
}

impl<'a, 'song, T, F> StatefulWidget for VirtualizedTable<'a, 'song, T, F>
where
    F: Fn(usize, &'song T) -> Row<'a>,
{
    type State = DirState<TableState>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
        let Some(viewport_len) = state.viewport_len() else {
            return;
        };
        let Some(map_fn) = self.map_fn.as_ref() else {
            return;
        };

        // Save original state and remove offset because ratatui's table will think that
        // we are rendering from item 0 to viewport_len, the rest will be ignored
        let original_offset = state.offset();
        let original_selected = state.inner.selected();
        *state.inner.offset_mut() = 0;
        state.select(original_selected.map(|v| v.saturating_sub(original_offset)), 0);

        let actual_rows = self
            .items
            .iter()
            .skip(original_offset)
            .take(viewport_len)
            .enumerate()
            .map(|(idx, item)| map_fn(idx + original_offset, item));
        let table = Table::new(actual_rows, self.column_widths)
            .row_highlight_style(self.row_highlight_style);

        StatefulWidget::render(table, area, buf, state.as_render_state_ref());

        // Restore the original state
        *state.inner.offset_mut() = original_offset;
        state.select(original_selected, 0);
    }
}
