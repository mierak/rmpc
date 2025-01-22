use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Padding, StatefulWidget},
};
use style::Styled;

use crate::{
    config::Config,
    ui::dirstack::{Dir, DirStack, DirStackItem},
};

#[derive(Debug)]
pub struct Browser<T: std::fmt::Debug + DirStackItem + Clone + Send> {
    state_type_marker: std::marker::PhantomData<T>,
    widths: Vec<u16>,
    config: &'static Config,
    border_style: Style,
    pub areas: [Rect; 3],
    filter_input_active: bool,
}

impl<T: std::fmt::Debug + DirStackItem + Clone + Send> Browser<T> {
    pub fn new(config: &'static Config) -> Self {
        Self {
            state_type_marker: std::marker::PhantomData,
            widths: config.theme.column_widths.to_vec(),
            config,
            border_style: config.as_border_style(),
            areas: [Rect::default(); 3],
            filter_input_active: false,
        }
    }

    pub fn set_filter_input_active(&mut self, value: bool) -> &mut Self {
        self.filter_input_active = value;
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

impl<'a, T> StatefulWidget for &mut Browser<T>
where
    T: std::fmt::Debug + DirStackItem<Item = ListItem<'a>> + Clone + Send,
{
    type State = DirStack<T>;

    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut Self::State,
    ) {
        let scrollbar_margin = if self.config.theme.draw_borders {
            let scrollbar_track = self.config.theme.scrollbar.symbols[0];
            Margin { vertical: 0, horizontal: scrollbar_track.is_empty().into() }
        } else {
            Margin { vertical: 0, horizontal: 0 }
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

        if self.widths[2] > 0 {
            self.areas[2] = preview_area;
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

            previous = previous.highlight_style(self.config.theme.current_item_style);

            let inner_block = block.inner(previous_area);
            self.areas[0] = inner_block;
            ratatui::widgets::StatefulWidget::render(
                previous,
                inner_block,
                buf,
                prev_state.as_render_state_ref(),
            );
            ratatui::widgets::Widget::render(block, previous_area, buf);
            ratatui::widgets::StatefulWidget::render(
                self.config.as_styled_scrollbar(),
                previous_area.inner(scrollbar_margin),
                buf,
                prev_state.as_scrollbar_state_ref(),
            );
        }
        if self.widths[1] > 0 {
            let title = state.current().filter().as_ref().map(|v| {
                format!("[FILTER]: {v}{} ", if self.filter_input_active { "█" } else { "" })
            });
            let Dir { items, state, .. } = state.current_mut();
            state.set_content_len(Some(items.len()));
            state.set_viewport_len(Some(current_area.height.into()));

            let block = {
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
            };
            let current = List::new(current)
                .highlight_style(self.config.theme.current_item_style)
                .style(self.config.as_text_style());

            let inner_block = block.inner(current_area);
            ratatui::widgets::StatefulWidget::render(
                current,
                inner_block,
                buf,
                state.as_render_state_ref(),
            );
            self.areas[1] = inner_block;
            ratatui::widgets::Widget::render(block, current_area, buf);
            ratatui::widgets::StatefulWidget::render(
                self.config.as_styled_scrollbar(),
                current_area.inner(scrollbar_margin),
                buf,
                state.as_scrollbar_state_ref(),
            );
        }
    }
}
