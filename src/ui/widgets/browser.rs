use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Scrollbar, ScrollbarOrientation, StatefulWidget};

use crate::ui::screens::dirstack::{DirStack, MatchesSearch};

#[derive(Debug)]
pub struct Browser<'a, T> {
    state_type_marker: std::marker::PhantomData<T>,
    widths: &'a [u16; 3],
    previous: &'a [ListItem<'a>],
    current: &'a [ListItem<'a>],
    preview: Option<Vec<ListItem<'a>>>,
}

impl<'a, T> Default for Browser<'a, T> {
    fn default() -> Self {
        Self {
            state_type_marker: std::marker::PhantomData,
            widths: &[20, 38, 42],
            previous: &[],
            current: &[],
            preview: None,
        }
    }
}

impl<'a, T> Browser<'a, T> {
    pub fn new() -> Self {
        Browser::default()
    }

    pub fn previous_items(mut self, previous: &'a [ListItem<'_>]) -> Self {
        self.previous = previous;
        self
    }
    pub fn current_items(mut self, current: &'a [ListItem<'_>]) -> Self {
        self.current = current;
        self
    }
    pub fn preview(mut self, preview: Option<Vec<ListItem<'a>>>) -> Self {
        self.preview = preview;
        self
    }

    pub fn widths(mut self, widths: &'a [u16; 3]) -> Self {
        self.widths = widths;
        self
    }
}

impl<T> StatefulWidget for Browser<'_, T>
where
    T: MatchesSearch + std::fmt::Debug,
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
            let preview = List::new(self.preview.unwrap_or(Vec::new()))
                .block(Block::default().borders(Borders::ALL))
                .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
            ratatui::widgets::Widget::render(preview, preview_area, buf);
        }

        {
            let (_, prev_state) = state.get_previous();
            prev_state.content_len(Some(u16::try_from(self.previous.len()).unwrap()));
            prev_state.viewport_len(Some(previous_area.height));

            let previous = List::new(self.previous)
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
        let title = state.filter.as_ref().map(|v| format!("[FILTER]: {v} "));
        {
            let (current_items, current_state) = state.get_current();
            current_state.content_len(Some(u16::try_from(current_items.len()).unwrap()));
            current_state.viewport_len(Some(current_area.height));

            let current = List::new(self.current)
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

            ratatui::widgets::StatefulWidget::render(current, current_area, buf, current_state.as_render_state_ref());
            ratatui::widgets::StatefulWidget::render(
                current_scrollbar,
                preview_area.inner(&ratatui::prelude::Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                buf,
                current_state.as_scrollbar_state_ref(),
            );
        }
    }
}
