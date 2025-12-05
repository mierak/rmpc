use enum_map::{Enum, EnumMap};
use itertools::Itertools;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Padding},
};
use style::Styled;

use crate::{
    ctx::Ctx,
    ui::dirstack::{Dir, DirStack, DirStackItem},
};

#[derive(Copy, Clone, Debug, Enum, Eq, PartialEq, Hash)]
pub enum BrowserArea {
    Previous = 0,
    Current = 1,
    Preview = 2,
    Scrollbar = 3,
}

#[derive(Debug)]
pub struct Browser<T: std::fmt::Debug + DirStackItem + Clone + Send> {
    state_type_marker: std::marker::PhantomData<T>,
    pub areas: EnumMap<BrowserArea, Rect>,
}

impl<T: std::fmt::Debug + DirStackItem + Clone + Send> Browser<T> {
    pub fn new() -> Self {
        Self { state_type_marker: std::marker::PhantomData, areas: EnumMap::default() }
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

impl<T> Browser<T>
where
    T: std::fmt::Debug + DirStackItem + Clone + Send,
{
    pub fn render(
        &mut self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut DirStack<T, ListState>,
        ctx: &Ctx,
    ) {
        let config = &ctx.config;
        let song_format = ctx.config.theme.browser_song_format.0.as_slice();
        let scrollbar_margin = match config.theme.scrollbar.as_ref() {
            Some(scrollbar) if config.theme.draw_borders => {
                let scrollbar_track = &scrollbar.symbols[0];
                Margin { vertical: 0, horizontal: scrollbar_track.is_empty().into() }
            }
            Some(_) | None => Margin { vertical: 0, horizontal: 0 },
        };
        let column_right_padding: u16 = config.theme.scrollbar.is_some().into();

        let current = state.current().to_list_items(song_format, ctx);

        let [previous_area, current_area, preview_area] = *Layout::horizontal([
            Constraint::Percentage(config.theme.column_widths[0]),
            Constraint::Percentage(config.theme.column_widths[1]),
            Constraint::Percentage(config.theme.column_widths[2]),
        ])
        .split(area) else {
            return;
        };

        self.areas[BrowserArea::Preview] = preview_area;
        if config.theme.column_widths[2] > 0 {
            let result = if let Some(current) = state.current().selected()
                && current.is_file()
            {
                let previews = current.to_file_preview(ctx);
                let mut result = Vec::new();
                for group in previews {
                    if let Some(name) = group.name {
                        let mut item = ListItem::new(name);
                        if let Some(style) = group.header_style {
                            item = item.style(style);
                        }
                        result.push(item);
                    }
                    result.extend(group.items);
                    result.push(ListItem::new(Span::raw("")));
                }

                result
            } else if state.current().selected().is_some() {
                let items = state.next_dir_items().map_or(Vec::new(), |p| {
                    p.iter()
                        .take(self.areas[BrowserArea::Preview].height as usize)
                        .map(|item| item.to_list_item_simple(ctx))
                        .collect_vec()
                });
                if let Some(next) = state.next_mut() {
                    next.state
                        .set_content_and_viewport_len(items.len(), previous_area.height.into());
                }
                items
            } else {
                Vec::new()
            };

            let preview = List::new(result).style(config.as_text_style());
            ratatui::widgets::Widget::render(preview, preview_area, buf);
        }

        if let Some(previous) = state.previous_mut()
            && config.theme.column_widths[0] > 0
        {
            let items = previous.to_list_items(song_format, ctx);
            let title = previous.filter().as_ref().map(|v| format!("[FILTER]: {v} "));
            let prev_state = &mut previous.state;
            prev_state.set_content_and_viewport_len(items.len(), previous_area.height.into());

            let mut previous = List::new(items).style(config.as_text_style());
            let mut block = if config.theme.draw_borders {
                Block::default()
                    .borders(Borders::RIGHT)
                    .border_style(config.as_border_style())
                    .padding(Padding::new(0, column_right_padding, 0, 0))
                    .border_set(LEFT_COLUMN_SYMBOLS)
            } else {
                Block::default().padding(Padding::new(1, column_right_padding, 0, 0))
            };
            if let Some(ref title) = title {
                block = block.title(title.clone().set_style(config.theme.borders_style));
            }

            previous = previous.highlight_style(config.theme.current_item_style);

            let inner_block = block.inner(previous_area);
            self.areas[BrowserArea::Previous] = inner_block;
            ratatui::widgets::StatefulWidget::render(
                previous,
                inner_block,
                buf,
                prev_state.as_render_state_ref(),
            );
            ratatui::widgets::Widget::render(block, previous_area, buf);
            if let Some(scrollbar) = config.as_styled_scrollbar()
                && prev_state.content_len().is_some_and(|l| l > 0)
            {
                ratatui::widgets::StatefulWidget::render(
                    scrollbar,
                    previous_area.inner(scrollbar_margin),
                    buf,
                    prev_state.as_scrollbar_state_ref(),
                );
            }
        }
        if config.theme.column_widths[1] > 0 {
            let title = state.current().filter().as_ref().map(|v| {
                format!("[FILTER]: {v}{} ", if ctx.input.is_insert_mode() { "█" } else { "" })
            });
            let Dir { items, state, .. } = state.current_mut();
            state.set_content_and_viewport_len(items.len(), current_area.height.into());

            let block = {
                let mut b = Block::default();
                if config.theme.draw_borders {
                    b = b
                        .borders(Borders::RIGHT)
                        .border_style(config.as_border_style())
                        .border_set(MIDDLE_COLUMN_SYMBOLS);
                }
                if let Some(ref title) = title {
                    b = b.title(title.clone().set_style(config.theme.borders_style));
                }
                b.padding(Padding::new(0, column_right_padding, 0, 0))
            };
            let current = List::new(current)
                .highlight_style(config.theme.current_item_style)
                .style(config.as_text_style());

            let inner_block = block.inner(current_area);
            ratatui::widgets::StatefulWidget::render(
                current,
                inner_block,
                buf,
                state.as_render_state_ref(),
            );
            self.areas[BrowserArea::Current] = inner_block;
            let scrollbar_area = current_area.inner(scrollbar_margin);
            self.areas[BrowserArea::Scrollbar] = scrollbar_area;
            ratatui::widgets::Widget::render(block, current_area, buf);
            if let Some(scrollbar) = config.as_styled_scrollbar() {
                ratatui::widgets::StatefulWidget::render(
                    scrollbar,
                    scrollbar_area,
                    buf,
                    state.as_scrollbar_state_ref(),
                );
            }
        }
    }
}
