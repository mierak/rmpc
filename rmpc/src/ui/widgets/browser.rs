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
    pub song_format: Option<Vec<Property<SongProperty>>>,
    pub column_titles: Option<[String; 3]>,
}
impl<T: std::fmt::Debug + DirStackItem + Clone + Send> Browser<T> {
    pub fn new() -> Self {
        Self {
            state_type_marker: std::marker::PhantomData,
            areas: EnumMap::default(),
            song_format: None,
            column_titles: None,
        }
    }

    pub fn with_song_format(mut self, format: Vec<Property<SongProperty>>) -> Self {
        self.song_format = Some(format);
        self
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
        let song_format: &[Property<SongProperty>] = self
            .song_format
            .as_deref()
            .unwrap_or(ctx.config.theme.browser_song_format.0.as_slice());
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
            let accent = config.theme.highlight_border_style.fg.unwrap_or(Color::Cyan);
            let warm = config.theme.highlighted_item_style.fg.unwrap_or(accent);
            let faint = config.theme.borders_style.fg.unwrap_or(Color::DarkGray);
            let pw = right_area.width as usize;
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
                    Style::default().fg(faint),
                )))
            };
            let sel_is_file = state.current().selected().map(DirStackItem::is_file);
            let mut result: Vec<ListItem> = Vec::new();
            match sel_is_file {
                // A song is selected: show its metadata, framed by a title/artist header.
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
                }
                // A directory is selected: if its contents are songs, render an
                // album-style preview (header + track list); otherwise list contents.
                Some(false) => {
                    let is_songs = state
                        .next_dir_items()
                        .is_some_and(|v| v.first().is_some_and(DirStackItem::is_file));
                    if is_songs {
                        let children: &[T] = state.next_dir_items().map_or(&[], Vec::as_slice);
                        let count = children.len();
                        let first = &children[0];
                        let album = prop(first, SongProperty::Album);
                        let artist = prop(first, SongProperty::Artist);
                        let genre = prop(first, SongProperty::Other("genre".to_string()));
                        let year = prop(first, SongProperty::Other("date".to_string()));
                        result.push(ListItem::new(Line::from(Span::styled(
                            if album.is_empty() { "Preview".to_string() } else { album },
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
                        let rows = (right_area.height as usize).saturating_sub(result.len());
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
                    } else {
                        let items: Vec<ListItem> = state.next_dir_items().map_or(Vec::new(), |p| {
                            p.iter()
                                .take(right_area.height as usize)
                                .map(|item| item.to_list_item_simple(ctx))
                                .collect_vec()
                        });
                        let len = items.len();
                        result = items;
                        if let Some(next) = state.next_mut() {
                            next.state.set_content_and_viewport_len(len, right_area.height.into());
                        }
                    }
                }
                None => {}
            }
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
