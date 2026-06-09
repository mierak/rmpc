use enum_map::{Enum, EnumMap};
use itertools::Itertools;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Padding},
};

use crate::{
    config::theme::properties::{Property, PropertyKindOrText, SongProperty},
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
    pub column_titles: Option<[String; 3]>,
}
impl<T: std::fmt::Debug + DirStackItem + Clone + Send> Browser<T> {
    pub fn new() -> Self {
        Self {
            state_type_marker: std::marker::PhantomData,
            areas: EnumMap::default(),
            column_titles: None,
        }
    }
}
const MIDDLE_COLUMN_SYMBOLS: symbols::border::Set = symbols::border::Set {
    top_right: symbols::line::NORMAL.horizontal_down,
    bottom_right: symbols::line::NORMAL.horizontal_up,
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

        let w_left = config.theme.column_widths[0].saturating_add(config.theme.column_widths[1]);
        let w_right = config.theme.column_widths[2];
        let [mut left_area, mut right_area] =
            *Layout::horizontal([Constraint::Percentage(w_left), Constraint::Percentage(w_right)])
                .split(area)
        else {
            return;
        };

        // No parent column in focus-left layout
        self.areas[BrowserArea::Previous] = Rect::default();

        // Render column titles if configured
        if let Some([t0, _t1, t2]) = &self.column_titles {
            let accent = config.theme.highlight_border_style.fg.unwrap_or(Color::Cyan);
            let title_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);
            if area.height > 0 {
                if w_left > 0 {
                    Line::styled(t0.as_str(), title_style)
                        .render(Rect { height: 1, ..left_area }, buf);
                    left_area.y += 1;
                    left_area.height = left_area.height.saturating_sub(1);
                }
                if w_right > 0 {
                    Line::styled(t2.as_str(), title_style)
                        .render(Rect { height: 1, ..right_area }, buf);
                    right_area.y += 1;
                    right_area.height = right_area.height.saturating_sub(1);
                }
            }
        }

        self.areas[BrowserArea::Preview] = right_area;
        if w_right > 0 {
            let result = if let Some(current) = state.current().selected()
                && current.is_file()
            {
                let previews = current.to_file_preview(ctx);
                let mut result = Vec::new();
                let accent = config.theme.highlight_border_style.fg.unwrap_or(Color::Cyan);
                let album = current.format(
                    &[Property {
                        kind: PropertyKindOrText::Property(SongProperty::Album),
                        style: None,
                        default: None,
                    }],
                    "",
                    ctx,
                );
                let artist = current.format(
                    &[Property {
                        kind: PropertyKindOrText::Property(SongProperty::Artist),
                        style: None,
                        default: None,
                    }],
                    "",
                    ctx,
                );
                result.push(ListItem::new(Line::from(vec![
                    Span::styled(album, Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                    Span::styled(" — ", Style::default().add_modifier(Modifier::DIM)),
                    Span::styled(artist, Style::default().fg(accent)),
                ])));
                result.push(ListItem::new(Span::raw("")));
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
                    next.state.set_content_and_viewport_len(items.len(), right_area.height.into());
                }
                items
            } else {
                Vec::new()
            };

            let preview = List::new(result).style(config.as_text_style());
            ratatui::widgets::Widget::render(preview, right_area, buf);
        }

        if w_left > 0 {
            let title = state.current().filter_text(left_area.width.saturating_sub(2), ctx);

            let Dir { items, state, .. } = state.current_mut();
            state.set_content_and_viewport_len(items.len(), left_area.height.into());

            let block = {
                let mut b = Block::default();
                if config.theme.draw_borders {
                    b = b
                        .borders(Borders::RIGHT)
                        .border_style(config.as_border_style())
                        .border_set(MIDDLE_COLUMN_SYMBOLS);
                }
                if let Some(title) = title {
                    b = b.title(title);
                }
                b.padding(Padding::new(0, column_right_padding, 0, 0))
            };
            let current = List::new(current).style(config.as_text_style());

            let inner_block = block.inner(left_area);
            ratatui::widgets::StatefulWidget::render(
                current,
                inner_block,
                buf,
                state.as_render_state_ref(),
            );
            self.areas[BrowserArea::Current] = inner_block;
            let scrollbar_area = left_area.inner(scrollbar_margin);
            self.areas[BrowserArea::Scrollbar] = scrollbar_area;
            ratatui::widgets::Widget::render(block, left_area, buf);
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
