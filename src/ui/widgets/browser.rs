use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Padding, StatefulWidget};
use style::Styled;

use crate::config::Config;
use crate::ui::utils::dirstack::{Dir, DirStack, DirStackItem};

#[derive(Debug)]
pub struct Browser<T: std::fmt::Debug + DirStackItem> {
    state_type_marker: std::marker::PhantomData<T>,
    widths: Vec<u16>,
    config: &'static Config,
    border_style: Style,
    pub areas: [Rect; 3],
}

impl<T: std::fmt::Debug + DirStackItem> Browser<T> {
    pub fn new(config: &'static Config) -> Self {
        Self {
            state_type_marker: std::marker::PhantomData,
            widths: config.theme.column_widths.to_vec(),
            config,
            border_style: config.as_border_style(),
            areas: [Rect::default(); 3],
        }
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

impl<'a, T> StatefulWidget for &mut Browser<T>
where
    T: std::fmt::Debug + DirStackItem<Item = ListItem<'a>>,
{
    type State = DirStack<T>;

    #[allow(clippy::unwrap_used)]
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer, state: &mut Self::State) {
        let scrollbar_margin = Margin {
            vertical: 0,
            horizontal: 0,
        };
        let previous = state.previous().to_list_items(self.config);
        let current = state.current().to_list_items(self.config);
        let preview = state.preview().cloned();

        let [previous_area, current_area, preview_area] = *Layout::horizontal([
            Constraint::Percentage(self.widths[0]),
            Constraint::Percentage(self.widths[1]),
            Constraint::Percentage(self.widths[2]),
        ])
        .split(area) else {
            return;
        };
        self.areas = [previous_area, current_area, preview_area];

        if self.widths[2] > 0 {
            let preview = List::new(preview.unwrap_or_default())
                .highlight_style(self.config.theme.current_item_style)
                .style(self.config.as_text_style());
            ratatui::widgets::Widget::render(preview, preview_area, buf);
        }

        if self.widths[0] > 0 {
            let title = state.previous().filter().as_ref().map(|v| format!("[FILTER]: {v} "));
            let prev_state = &mut state.previous_mut().state;
            prev_state.set_content_len(Some(previous.len()));
            prev_state.set_viewport_len(Some(previous_area.height.into()));

            let mut previous = List::new(previous).style(self.config.as_text_style());
            let mut block = if self.config.theme.draw_borders {
                Block::default()
                    .borders(Borders::RIGHT)
                    .border_style(self.border_style)
                    .padding(Padding::new(0, 1, 0, 0))
                    .border_set(LEFT_COLUMN_SYMBOLS)
            } else {
                Block::default().padding(Padding::new(1, 2, 0, 0))
            };
            if let Some(ref title) = title {
                block = block.title(title.clone().set_style(self.config.theme.borders_style));
            }

            previous = previous
                .block(block)
                .highlight_style(self.config.theme.current_item_style);

            ratatui::widgets::StatefulWidget::render(previous, previous_area, buf, prev_state.as_render_state_ref());
            ratatui::widgets::StatefulWidget::render(
                self.config.as_styled_scrollbar(),
                previous_area.inner(scrollbar_margin),
                buf,
                prev_state.as_scrollbar_state_ref(),
            );
        }
        if self.widths[1] > 0 {
            let title = state.current().filter().as_ref().map(|v| format!("[FILTER]: {v} "));
            let Dir { items, state, .. } = state.current_mut();
            state.set_content_len(Some(items.len()));
            state.set_viewport_len(Some(current_area.height.into()));

            let current = List::new(current)
                .block({
                    let mut b = Block::default();
                    if self.config.theme.draw_borders {
                        b = b
                            .borders(Borders::RIGHT)
                            .border_style(self.border_style)
                            .border_set(MIDDLE_COLUMN_SYMBOLS);
                    }
                    if let Some(ref title) = title {
                        b = b.title(title.clone().set_style(self.config.theme.borders_style));
                    }
                    b.padding(Padding::new(0, 1, 0, 0))
                })
                .highlight_style(self.config.theme.current_item_style)
                .style(self.config.as_text_style());

            ratatui::widgets::StatefulWidget::render(current, current_area, buf, state.as_render_state_ref());
            ratatui::widgets::StatefulWidget::render(
                self.config.as_styled_scrollbar(),
                current_area.inner(scrollbar_margin),
                buf,
                state.as_scrollbar_state_ref(),
            );
        }
    }
}
