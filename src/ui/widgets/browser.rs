use itertools::Itertools;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, Scrollbar, ScrollbarOrientation, StatefulWidget};

use crate::config::SymbolsConfig;
use crate::ui::utils::dirstack::{Dir, DirStack, DirStackItem};

#[derive(Debug)]
pub struct Browser<'a, T: std::fmt::Debug + DirStackItem> {
    state_type_marker: std::marker::PhantomData<T>,
    widths: &'a [u16; 3],
    symbols: &'a SymbolsConfig,
}

impl<'a, T: std::fmt::Debug + DirStackItem> Browser<'a, T> {
    pub fn new(symbols: &'a SymbolsConfig) -> Self {
        Self {
            state_type_marker: std::marker::PhantomData,
            widths: &[20, 38, 42],
            symbols,
        }
    }

    pub fn set_widths(mut self, widths: &'a [u16; 3]) -> Self {
        self.widths = widths;
        self
    }
}
const MIDDLE_COLUMN_SYMBOLS: symbols::border::Set = symbols::border::Set {
    top_right: symbols::line::NORMAL.horizontal_down,
    bottom_right: symbols::line::NORMAL.horizontal_up,
    ..symbols::border::PLAIN
};

const LEFT_COLUMN_SYMBOLS: symbols::border::Set = symbols::border::Set {
    bottom_right: symbols::line::NORMAL.horizontal_up,
    top_right: symbols::line::NORMAL.horizontal_down,
    ..symbols::border::PLAIN
};

impl<T> StatefulWidget for Browser<'_, T>
where
    T: std::fmt::Debug + DirStackItem,
{
    type State = DirStack<T>;

    #[allow(clippy::unwrap_used)]
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer, state: &mut Self::State) {
        let previous = state
            .previous()
            .items
            .iter()
            .enumerate()
            .map(|(idx, v)| v.to_list_item(self.symbols, state.previous().marked().contains(&idx)))
            .collect_vec();
        let current = state
            .current()
            .items
            .iter()
            .enumerate()
            .map(|(idx, v)| v.to_list_item(self.symbols, state.current().marked().contains(&idx)))
            .collect_vec();
        let preview = state.preview().cloned();

        let [previous_area, current_area, preview_area] = *Layout::default()
            .direction(ratatui::prelude::Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(self.widths[0]),
                    Constraint::Percentage(self.widths[1]),
                    Constraint::Percentage(self.widths[2]),
                ]
                .as_ref(),
            )
            .split(area)
        else {
            return;
        };

        {
            let preview = List::new(preview.unwrap_or_default())
                .block(Block::default().borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM))
                .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
            ratatui::widgets::Widget::render(preview, preview_area, buf);
        }

        {
            let prev_state = &mut state.previous_mut().state;
            prev_state.set_content_len(Some(previous.len()));
            prev_state.set_viewport_len(Some(previous_area.height.into()));

            let previous = List::new(previous)
                .block(Block::default().borders(Borders::ALL).border_set(LEFT_COLUMN_SYMBOLS))
                .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
            let previous_scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .track_symbol(Some("│"))
                .end_symbol(Some("↓"))
                .track_style(Style::default().fg(Color::White).bg(Color::Black))
                .begin_style(Style::default().fg(Color::White).bg(Color::Black))
                .end_style(Style::default().fg(Color::White).bg(Color::Black))
                .thumb_style(Style::default().fg(Color::Blue));

            ratatui::widgets::StatefulWidget::render(previous, previous_area, buf, prev_state.as_render_state_ref());
            ratatui::widgets::StatefulWidget::render(
                previous_scrollbar,
                previous_area.inner(&ratatui::prelude::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                buf,
                prev_state.as_scrollbar_state_ref(),
            );
        }
        let title = state.current().filter.as_ref().map(|v| format!("[FILTER]: {v} "));
        {
            let Dir { items, state, .. } = state.current_mut();
            state.set_content_len(Some(items.len()));
            state.set_viewport_len(Some(current_area.height.into()));

            let current = List::new(current)
                .block({
                    let mut b = Block::default()
                        .borders(Borders::TOP | Borders::BOTTOM | Borders::RIGHT)
                        .border_set(MIDDLE_COLUMN_SYMBOLS);
                    if let Some(ref title) = title {
                        b = b.title(title.clone().blue());
                    }
                    b
                })
                .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
            let current_scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .track_symbol(Some("│"))
                .end_symbol(Some("↓"))
                .track_style(Style::default().fg(Color::White).bg(Color::Black))
                .begin_style(Style::default().fg(Color::White).bg(Color::Black))
                .end_style(Style::default().fg(Color::White).bg(Color::Black))
                .thumb_style(Style::default().fg(Color::Blue));

            ratatui::widgets::StatefulWidget::render(current, current_area, buf, state.as_render_state_ref());
            ratatui::widgets::StatefulWidget::render(
                current_scrollbar,
                current_area.inner(&ratatui::prelude::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                buf,
                state.as_scrollbar_state_ref(),
            );
        }
    }
}
