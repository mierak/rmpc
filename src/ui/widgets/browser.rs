use itertools::Itertools;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, Padding, StatefulWidget};

use crate::config::Config;
use crate::ui::utils::dirstack::{Dir, DirStack, DirStackItem};

#[derive(Debug)]
pub struct Browser<'a, T: std::fmt::Debug + DirStackItem> {
    state_type_marker: std::marker::PhantomData<T>,
    widths: &'a [u16; 3],
    config: &'a Config,
    border_style: Style,
}

impl<'a, T: std::fmt::Debug + DirStackItem> Browser<'a, T> {
    pub fn new(config: &'a Config) -> Self {
        Self {
            state_type_marker: std::marker::PhantomData,
            widths: &[20, 38, 42],
            config,
            border_style: Style::default(),
        }
    }

    pub fn set_widths(mut self, widths: &'a [u16; 3]) -> Self {
        self.widths = widths;
        self
    }

    pub fn set_border_style(mut self, border_style: Style) -> Self {
        self.border_style = border_style;
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
        let scrollbar_margin = Margin {
            vertical: 0,
            horizontal: 0,
        };
        let previous = state
            .previous()
            .items
            .iter()
            .enumerate()
            .map(|(idx, v)| v.to_list_item(&self.config.ui.symbols, state.previous().marked().contains(&idx)))
            .collect_vec();
        let current = state
            .current()
            .items
            .iter()
            .enumerate()
            .map(|(idx, v)| v.to_list_item(&self.config.ui.symbols, state.current().marked().contains(&idx)))
            .collect_vec();
        let preview = state.preview().cloned();

        let [previous_area, current_area, preview_area] = *Layout::horizontal([
            Constraint::Percentage(self.widths[0]),
            Constraint::Percentage(self.widths[1]),
            Constraint::Percentage(self.widths[2]),
        ])
        .split(area) else {
            return;
        };

        {
            let preview = List::new(preview.unwrap_or_default())
                .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());
            ratatui::widgets::Widget::render(preview, preview_area, buf);
        }

        {
            let prev_state = &mut state.previous_mut().state;
            prev_state.set_content_len(Some(previous.len()));
            prev_state.set_viewport_len(Some(previous_area.height.into()));

            let mut previous = List::new(previous);
            if self.config.ui.draw_borders {
                previous = previous.block(
                    Block::default()
                        .borders(Borders::RIGHT)
                        .border_style(self.border_style)
                        .padding(Padding::new(0, 2, 0, 0))
                        .border_set(LEFT_COLUMN_SYMBOLS),
                );
            } else {
                previous = previous.block(Block::default().padding(Padding::new(1, 2, 0, 0)));
            };
            previous = previous.highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());

            ratatui::widgets::StatefulWidget::render(previous, previous_area, buf, prev_state.as_render_state_ref());
            ratatui::widgets::StatefulWidget::render(
                self.config.as_styled_scrollbar(),
                previous_area.inner(&scrollbar_margin),
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
                    let mut b = Block::default();
                    if self.config.ui.draw_borders {
                        b = b
                            .borders(Borders::RIGHT)
                            .border_style(self.border_style)
                            .border_set(MIDDLE_COLUMN_SYMBOLS);
                    }
                    if let Some(ref title) = title {
                        b = b.title(title.clone().blue());
                    }
                    b.padding(Padding::new(1, 2, 0, 0))
                })
                .highlight_style(Style::default().bg(Color::Blue).fg(Color::Black).bold());

            ratatui::widgets::StatefulWidget::render(current, current_area, buf, state.as_render_state_ref());
            ratatui::widgets::StatefulWidget::render(
                self.config.as_styled_scrollbar(),
                current_area.inner(&scrollbar_margin),
                buf,
                state.as_scrollbar_state_ref(),
            );
        }
    }
}
