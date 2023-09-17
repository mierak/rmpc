use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, Scrollbar, ScrollbarOrientation, StatefulWidget};

use crate::config::SymbolsConfig;
use crate::ui::screens::browser::ToListItems;
use crate::ui::screens::dirstack::{DirStack, MatchesSearch};

#[derive(Debug)]
pub struct Browser<'a, T> {
    state_type_marker: std::marker::PhantomData<T>,
    pub symbols: &'a SymbolsConfig,
    pub widths: &'a [u16; 3],
}

impl<'a, T> Browser<'a, T> {
    pub fn new(symbols: &'a SymbolsConfig, widths: &'a [u16; 3]) -> Self {
        Self {
            symbols,
            state_type_marker: std::marker::PhantomData,
            widths,
        }
    }
}

impl<T> StatefulWidget for Browser<'_, T>
where
    T: MatchesSearch + std::fmt::Debug,
    Vec<T>: ToListItems,
{
    type State = DirStack<T>;

    #[allow(clippy::unwrap_used)]
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer, state: &mut Self::State) {
        let [previous_area, current_area, preview_area] = *Layout::default()
            .direction(ratatui::prelude::Direction::Horizontal)
            .constraints([
                         Constraint::Percentage(self.widths[0]),
                         Constraint::Percentage(self.widths[1]),
                         Constraint::Percentage(self.widths[2]),
            ].as_ref())
            .split(area) else { return  };

        {
            let preview = List::new(state.preview.clone())
                .block(Block::default().borders(Borders::ALL))
                .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
            ratatui::widgets::Widget::render(preview, preview_area, buf);
        }

        {
            let (prev_items, prev_state) = state.previous();
            let prev_items = prev_items.to_listitems(self.symbols);
            prev_state.content_len(Some(u16::try_from(prev_items.len()).unwrap()));
            prev_state.viewport_len(Some(previous_area.height));

            let previous = List::new(prev_items)
                .block(Block::default().borders(Borders::ALL))
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

            ratatui::widgets::StatefulWidget::render(previous, previous_area, buf, &mut prev_state.inner);
            ratatui::widgets::StatefulWidget::render(
                previous_scrollbar,
                previous_area.inner(&ratatui::prelude::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                buf,
                &mut prev_state.scrollbar_state,
            );
        }
        let title = state.filter.as_ref().map(|v| format!("[FILTER]: {v} "));
        {
            let (current_items, current_state) = state.current();
            current_state.content_len(Some(u16::try_from(current_items.len()).unwrap()));
            current_state.viewport_len(Some(current_area.height));

            let current = List::new(current_items.to_listitems(self.symbols))
                .block({
                    let mut b = Block::default().borders(Borders::TOP | Borders::BOTTOM);
                    if let Some(ref title) = title {
                        b = b.title(title.blue());
                    }
                    b
                })
                .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
            let current_scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalLeft)
                .begin_symbol(Some("↑"))
                .track_symbol(Some("│"))
                .end_symbol(Some("↓"))
                .track_style(Style::default().fg(Color::White).bg(Color::Black))
                .begin_style(Style::default().fg(Color::White).bg(Color::Black))
                .end_style(Style::default().fg(Color::White).bg(Color::Black))
                .thumb_style(Style::default().fg(Color::Blue));

            ratatui::widgets::StatefulWidget::render(current, current_area, buf, &mut current_state.inner);
            ratatui::widgets::StatefulWidget::render(
                current_scrollbar,
                preview_area.inner(&ratatui::prelude::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                buf,
                &mut current_state.scrollbar_state,
            );
        }
    }
}
