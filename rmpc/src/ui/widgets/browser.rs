use enum_map::{Enum, EnumMap};
use itertools::Itertools;
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, List, ListItem, ListState, Padding},
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
    Cover = 4,
}

#[derive(Debug)]
pub struct Browser<T: std::fmt::Debug + DirStackItem + Clone + Send> {
    state_type_marker: std::marker::PhantomData<T>,
    pub areas: EnumMap<BrowserArea, Rect>,
    pub column_titles: Option<[String; 3]>,
    /// Per-pane override of `theme.column_widths` (e.g. Playlists uses a hidden
    /// parent column + narrow list + wide song preview).
    pub widths: Option<[u16; 3]>,
    /// When true, the preview column reserves a `Cover` rect for an image
    /// facade.
    pub preview_cover: bool,
}
impl<T: std::fmt::Debug + DirStackItem + Clone + Send> Browser<T> {
    pub fn new() -> Self {
        Self {
            state_type_marker: std::marker::PhantomData,
            areas: EnumMap::default(),
            column_titles: None,
            widths: None,
            preview_cover: false,
        }
    }
}

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

        let current_items = state.current().to_list_items(song_format, ctx);
        let cw = self.widths.unwrap_or(config.theme.column_widths);
        let col_titles = self.column_titles.clone();
        let accent = config.theme.highlight_border_style.fg.unwrap_or(Color::Cyan);
        let title_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);
        let focus_border = config.as_focused_border_style();
        let dim_border = config.as_border_style();
        let box_title = |idx: usize, fallback: Option<String>| -> Option<String> {
            col_titles.as_ref().map(|t| t[idx].clone()).filter(|s| !s.is_empty()).or(fallback)
        };
        let [previous_area, current_area, preview_area] = *Layout::horizontal([
            Constraint::Percentage(cw[0]),
            Constraint::Percentage(cw[1]),
            Constraint::Percentage(cw[2]),
        ])
        .spacing(1)
        .split(area) else {
            return;
        };
        // ---- Preview column ----
        self.areas[BrowserArea::Preview] = preview_area;
        self.areas[BrowserArea::Cover] = Rect::default();
        if cw[2] > 0 {
            let pblock = {
                let mut b =
                    Block::bordered().border_type(BorderType::Rounded).border_style(dim_border);
                if let Some(t) = box_title(2, Some(" Preview".to_string())) {
                    b = b.title(t).title_style(title_style);
                }
                b
            };
            let inner = pblock.inner(preview_area);
            ratatui::widgets::Widget::render(pblock, preview_area, buf);
            self.areas[BrowserArea::Preview] = inner;
            let accent = config.theme.highlight_border_style.fg.unwrap_or(Color::Cyan);
            let warm = config.theme.highlighted_item_style.fg.unwrap_or(accent);
            let faint = config.theme.preview_label_style.fg.unwrap_or(Color::Gray);
            let rule = config.theme.borders_style.fg.unwrap_or(Color::DarkGray);
            let pw = inner.width as usize;
            let prop = |item: &T, sp: SongProperty| {
                item.format(
                    &[Property {
                        kind: PropertyKindOrText::Property(sp),
                        style: None,
                        default: None,
                    }],
                    "",
                    ctx,
                )
            };
            let sep = || {
                ListItem::new(Line::from(Span::styled(
                    "\u{2500}".repeat(pw),
                    Style::default().fg(rule),
                )))
            };
            let sel_is_file = state.current().selected().map(DirStackItem::is_file);
            let sel_name = state
                .current()
                .selected()
                .map(|s| prop(s, SongProperty::Title))
                .unwrap_or_default();
            let mut result: Vec<ListItem> = Vec::new();
            let mut has_song_preview = false;
            match sel_is_file {
                Some(true) => {
                    if let Some(sel) = state.current().selected() {
                        let album = prop(sel, SongProperty::Album);
                        let title = prop(sel, SongProperty::Title);
                        let artist = prop(sel, SongProperty::Artist);
                        result.push(ListItem::new(Line::from(Span::styled(
                            if album.is_empty() { title } else { album },
                            Style::default().fg(accent).add_modifier(Modifier::BOLD),
                        ))));
                        if !artist.is_empty() {
                            result.push(ListItem::new(Line::from(Span::styled(
                                artist,
                                Style::default().fg(warm),
                            ))));
                        }
                        result.push(sep());
                        for group in sel.to_file_preview(ctx) {
                            if let Some(name) = group.name {
                                let mut item = ListItem::new(name);
                                if let Some(style) = group.header_style {
                                    item = item.style(style);
                                }
                                result.push(item);
                            }
                            result.extend(group.items);
                        }
                    }
                    has_song_preview = true;
                }
                Some(false) => {
                    let is_songs = state
                        .next_dir_items()
                        .is_some_and(|v| v.first().is_some_and(DirStackItem::is_file));
                    if is_songs {
                        let children: &[T] = state.next_dir_items().map_or(&[], Vec::as_slice);
                        let count = children.len();
                        let first = &children[0];
                        let artist = prop(first, SongProperty::Artist);
                        let genre = prop(first, SongProperty::Other("genre".to_string()));
                        let year = prop(first, SongProperty::Other("date".to_string()));
                        result.push(ListItem::new(Line::from(Span::styled(
                            if sel_name.is_empty() { "Preview".to_string() } else { sel_name },
                            Style::default().fg(accent).add_modifier(Modifier::BOLD),
                        ))));
                        if !artist.is_empty() {
                            result.push(ListItem::new(Line::from(Span::styled(
                                artist,
                                Style::default().fg(warm),
                            ))));
                        }
                        let mut meta: Vec<String> = Vec::new();
                        if !genre.is_empty() {
                            meta.push(genre);
                        }
                        if !year.is_empty() {
                            meta.push(year);
                        }
                        meta.push(format!("{count} tracks"));
                        result.push(ListItem::new(Line::from(Span::styled(
                            meta.join(" \u{b7} "),
                            Style::default().fg(faint),
                        ))));
                        result.push(sep());
                        let rows = (inner.height as usize).saturating_sub(result.len());
                        for song in children.iter().take(rows) {
                            let track = prop(song, SongProperty::Track);
                            let title_raw = prop(song, SongProperty::Title);
                            let title = if title_raw.is_empty() {
                                prop(song, SongProperty::Filename)
                            } else {
                                title_raw
                            };
                            let dur = prop(song, SongProperty::Duration);
                            let dur_w = dur.chars().count();
                            let head_w = 4usize;
                            let title_max = pw.saturating_sub(head_w + dur_w + 1);
                            let title_disp: String = title.chars().take(title_max).collect();
                            let used = head_w + title_disp.chars().count() + dur_w;
                            let pad = pw.saturating_sub(used).max(1);
                            result.push(ListItem::new(Line::from(vec![
                                Span::styled(format!("{track:>3} "), Style::default().fg(faint)),
                                Span::styled(title_disp, config.as_text_style()),
                                Span::raw(" ".repeat(pad)),
                                Span::styled(dur, Style::default().fg(faint)),
                            ])));
                        }
                        has_song_preview = true;
                    } else {
                        let items: Vec<ListItem> = state.next_dir_items().map_or(Vec::new(), |p| {
                            p.iter()
                                .take(inner.height as usize)
                                .map(|item| item.to_list_item_simple(ctx))
                                .collect_vec()
                        });
                        let len = items.len();
                        result = items;
                        if let Some(next) = state.next_mut() {
                            next.state.set_content_and_viewport_len(len, inner.height.into());
                        }
                    }
                }
                None => {}
            }
            let render_area = if self.preview_cover && has_song_preview {
                let cover = Rect {
                    x: inner.x,
                    y: inner.y,
                    width: 14.min(inner.width),
                    height: 7.min(inner.height),
                };
                self.areas[BrowserArea::Cover] = cover;
                Rect {
                    x: inner.x,
                    y: inner.y + cover.height,
                    width: inner.width,
                    height: inner.height.saturating_sub(cover.height),
                }
            } else {
                inner
            };
            let preview = List::new(result).style(config.as_text_style());
            ratatui::widgets::Widget::render(preview, render_area, buf);
        }
        // ---- Previous (parent) column ----
        self.areas[BrowserArea::Previous] = Rect::default();
        if cw[0] > 0
            && let Some(previous) = state.previous_mut()
        {
            let items = previous.to_list_items(song_format, ctx);
            let title = previous.filter_text(previous_area.width, ctx);
            let prev_state = &mut previous.state;
            prev_state.set_content_and_viewport_len(items.len(), previous_area.height.into());
            let previous = List::new(items).style(config.as_text_style());
            let mut block = if config.theme.draw_borders {
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .border_style(dim_border)
                    .padding(Padding::new(0, column_right_padding, 0, 0))
            } else {
                Block::default().padding(Padding::new(1, column_right_padding, 0, 0))
            };
            if let Some(t) = col_titles.as_ref().map(|x| x[0].clone()).filter(|s| !s.is_empty()) {
                block = block.title(t).title_style(title_style);
            } else if let Some(title) = title {
                block = block.title(title);
            }
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
        // ---- Current (focused) column ----
        if cw[1] > 0 {
            let title = state.current().filter_text(current_area.width.saturating_sub(2), ctx);
            let Dir { items, state, .. } = state.current_mut();
            state.set_content_and_viewport_len(items.len(), current_area.height.into());
            let block = {
                let mut b = if config.theme.draw_borders {
                    Block::bordered()
                        .border_type(BorderType::Rounded)
                        .border_style(focus_border)
                        .padding(Padding::new(0, column_right_padding, 0, 0))
                } else {
                    Block::default().padding(Padding::new(0, column_right_padding, 0, 0))
                };
                if let Some(t) = col_titles.as_ref().map(|x| x[1].clone()).filter(|s| !s.is_empty())
                {
                    b = b.title(t).title_style(title_style);
                } else if let Some(title) = title {
                    b = b.title(title);
                }
                b
            };
            let current = List::new(current_items).style(config.as_text_style());
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
