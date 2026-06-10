use std::collections::HashMap;

use anyhow::{Context, Result};
use image::RgbaImage;
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, List, ListItem, ListState},
};
use rmpc_mpd::{
    client::Client,
    filter::{Filter, FilterKind, Tag},
    mpd_client::MpdClient,
};

use super::{
    Pane,
    gradient_art::{paint_cover, seed_of},
};
use crate::{
    config::{
        album_art::ImageMethod,
        keys::{
            CommonAction,
            actions::{AutoplayKind, Position},
        },
        tabs::PaneType,
    },
    ctx::Ctx,
    shared::{
        keys::ActionEvent,
        macros::modal,
        mouse_event::{MouseEvent, MouseEventKind},
        mpd_client_ext::{Enqueue, MpdClientExt},
        mpd_query::MpdQueryResult,
    },
    ui::{UiEvent, image::facade::AlbumArtFacade, modals::menu::modal::MenuModal},
};

const INIT: &str = "albums_grid_init";
const COVERS: &str = "albums_grid_covers";
const SEL_COVER: &str = "albums_grid_sel_cover";

// card geometry (cells)
const CARD_W: u16 = 18;
const GAP_X: u16 = 2;
const COVER_H: u16 = 8;
const CARD_H: u16 = COVER_H + 4; // cover + 3 label rows + spacing row

/// Album metadata stored alongside the cover image.
#[derive(Debug, Clone)]
struct AlbumEntry {
    name: String,
    artist: Option<String>,
    year: Option<String>,
    genre: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AlbumsView {
    #[default]
    Grid,
    List,
}

/// A responsive grid of generative album covers — the design's Albums screen.
///
/// Each album gets a procedural cover painted with [`paint_cover`] (sub-cell
/// half-block gradient, seeded by the album name), laid out in a grid that
/// reflows to the pane width. Arrow keys / hjkl move the selection, Enter
/// enqueues the album.
#[derive(Debug)]
pub struct AlbumsGridPane {
    albums: Vec<AlbumEntry>,
    selected: usize,
    scroll_row: usize,
    cols: usize,
    rows_visible: usize,
    area: Rect,
    covers: HashMap<String, RgbaImage>,
    /// Facade used to render the *selected* album's cover crisply via the
    /// terminal's image protocol (kitty/sixel/iterm/ueberzug). Other cards stay
    /// half-block. Only driven when `crisp` is set.
    album_art: AlbumArtFacade,
    crisp: bool,
    has_cover: bool,
    shown_album: Option<String>,
    initialized: bool,
    view: AlbumsView,
}

impl AlbumsGridPane {
    pub fn new(ctx: &Ctx) -> Self {
        let crisp = matches!(
            ctx.config.album_art.method,
            ImageMethod::Kitty
                | ImageMethod::Iterm2
                | ImageMethod::Sixel
                | ImageMethod::UeberzugWayland
                | ImageMethod::UeberzugX11
        );
        Self {
            albums: Vec::new(),
            selected: 0,
            scroll_row: 0,
            cols: 1,
            rows_visible: 1,
            area: Rect::default(),
            covers: HashMap::new(),
            album_art: AlbumArtFacade::new(ctx),
            crisp,
            has_cover: false,
            shown_album: None,
            initialized: false,
            view: AlbumsView::default(),
        }
    }

    fn enqueue_selected(&self, ctx: &Ctx) {
        let Some(name) = self.albums.get(self.selected).map(|e| e.name.clone()) else {
            return;
        };
        Client::resolve_and_enqueue(
            ctx,
            vec![Enqueue::Find { filter: vec![(Tag::Album, FilterKind::Exact, name)] }],
            Position::EndOfQueue,
            AutoplayKind::None,
            None,
            None,
        );
    }

    fn album_enqueue(name: String) -> Vec<Enqueue> {
        vec![Enqueue::Find { filter: vec![(Tag::Album, FilterKind::Exact, name)] }]
    }

    fn open_context_menu(&self, ctx: &Ctx) {
        let Some(name) = self.albums.get(self.selected).map(|e| e.name.clone()) else {
            return;
        };
        let (n1, n2, n3) = (name.clone(), name.clone(), name);
        let modal = MenuModal::new(ctx)
            .list_section(ctx, move |section| {
                Some(
                    section
                        .item("Add to queue", move |ctx| {
                            Client::resolve_and_enqueue(
                                ctx,
                                Self::album_enqueue(n1),
                                Position::EndOfQueue,
                                AutoplayKind::None,
                                None,
                                None,
                            );
                            Ok(())
                        })
                        .item("Add & play", move |ctx| {
                            Client::resolve_and_enqueue(
                                ctx,
                                Self::album_enqueue(n2),
                                Position::EndOfQueue,
                                AutoplayKind::First,
                                None,
                                None,
                            );
                            Ok(())
                        })
                        .item("Replace queue & play", move |ctx| {
                            Client::resolve_and_enqueue(
                                ctx,
                                Self::album_enqueue(n3),
                                Position::Replace,
                                AutoplayKind::First,
                                None,
                                None,
                            );
                            Ok(())
                        }),
                )
            })
            .list_section(ctx, |section| Some(section.item("Cancel", |_ctx| Ok(()))))
            .build();
        modal!(ctx, modal);
    }

    fn card_at(&self, x: u16, y: u16) -> Option<usize> {
        let (pad_x, pad_y) = (1u16, 1u16);
        if x < self.area.x + pad_x || y < self.area.y + pad_y {
            return None;
        }
        let rel_x = x - (self.area.x + pad_x);
        let rel_y = y - (self.area.y + pad_y);
        let col = (rel_x / (CARD_W + GAP_X)) as usize;
        if col >= self.cols || rel_x % (CARD_W + GAP_X) >= CARD_W {
            return None;
        }
        let row = self.scroll_row + (rel_y / CARD_H) as usize;
        let idx = row * self.cols.max(1) + col;
        (idx < self.albums.len()).then_some(idx)
    }

    /// Fetch a real cover for every album: a representative song's album art,
    /// decoded and downscaled to the card's sub-pixel size on the client
    /// thread.
    fn fetch_covers(&self, ctx: &Ctx) {
        let albums = self.albums.clone();
        let order = ctx.config.album_art.order;
        let (tw, th) = (u32::from(CARD_W), u32::from(COVER_H) * 2);
        ctx.query().id(COVERS).replace_id(COVERS).target(PaneType::AlbumsGrid).query(
            move |client| {
                type Meta = (Option<String>, Option<String>, Option<String>, Option<RgbaImage>);
                let mut out: Vec<(AlbumEntry, Option<RgbaImage>)> =
                    Vec::with_capacity(albums.len());
                for album in albums {
                    let name = album.name.clone();
                    let (artist, year, genre, cover) = (|| -> Result<Meta> {
                        let Some(song) =
                            client.find_one(&[Filter::new(Tag::Album, name.as_str())])?
                        else {
                            return Ok((None, None, None, None));
                        };
                        let artist = song.metadata.get("artist").map(|v| v.first().to_owned());
                        let year = song.metadata.get("date").map(|v| v.first().to_owned());
                        let genre = song.metadata.get("genre").map(|v| v.first().to_owned());
                        let Some(bytes) = client.find_album_art(&song.file, order)? else {
                            return Ok((artist, year, genre, None));
                        };
                        let img = image::load_from_memory(&bytes)?.to_rgba8();
                        Ok((
                            artist,
                            year,
                            genre,
                            Some(image::imageops::resize(
                                &img,
                                tw,
                                th,
                                image::imageops::FilterType::Lanczos3,
                            )),
                        ))
                    })()
                    .unwrap_or((None, None, None, None));
                    out.push((AlbumEntry { name, artist, year, genre }, cover));
                }
                Ok(MpdQueryResult::Any(Box::new(out)))
            },
        );
    }

    /// Cover rect of the currently-selected card, if it is visible.
    fn selected_cover_rect(&self) -> Option<Rect> {
        let cols = self.cols.max(1);
        let row = self.selected / cols;
        if row < self.scroll_row || row >= self.scroll_row + self.rows_visible.max(1) {
            return None;
        }
        let col = self.selected % cols;
        let cx = self.area.x + 1 + (col as u16) * (CARD_W + GAP_X);
        let cy = self.area.y + 1 + ((row - self.scroll_row) as u16) * CARD_H;
        let area_bottom = self.area.y.saturating_add(self.area.height);
        if cy >= area_bottom {
            return None;
        }
        Some(Rect::new(cx, cy, CARD_W, COVER_H.min(area_bottom - cy)))
    }

    /// Fetch the selected album's cover bytes and hand them to the facade.
    fn fetch_selected_cover(&mut self, ctx: &Ctx) {
        if !self.crisp {
            return;
        }
        let Some(album) = self.albums.get(self.selected).map(|e| e.name.clone()) else {
            return;
        };
        if self.shown_album.as_ref() == Some(&album) {
            return;
        }
        self.shown_album = Some(album.clone());
        let order = ctx.config.album_art.order;
        ctx.query().id(SEL_COVER).replace_id(SEL_COVER).target(PaneType::AlbumsGrid).query(
            move |client| {
                let cover = (|| -> Result<Option<Vec<u8>>> {
                    let Some(song) = client.find_one(&[Filter::new(Tag::Album, album.as_str())])?
                    else {
                        return Ok(None);
                    };
                    Ok(client.find_album_art(&song.file, order)?)
                })()
                .unwrap_or(None);
                Ok(MpdQueryResult::AlbumArt(cover))
            },
        );
    }

    fn render_list(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) {
        let theme = &ctx.config.theme;
        let accent = theme.highlight_border_style.fg.unwrap_or(Color::Cyan);
        let muted = theme.preview_label_style.fg.unwrap_or(Color::Gray);
        let text = theme.text_color.unwrap_or(Color::White);
        let inner = area;
        let items: Vec<ListItem> = self
            .albums
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let selected = i == self.selected;
                let mut spans = vec![
                    if selected {
                        Span::styled("\u{258e} ", Style::default().fg(accent))
                    } else {
                        Span::raw("  ")
                    },
                    Span::styled(
                        e.name.clone(),
                        Style::default().fg(text).add_modifier(if selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                    ),
                ];
                if let Some(a) = &e.artist {
                    spans.push(Span::styled(format!("   {a}"), Style::default().fg(muted)));
                }
                if let Some(y) = &e.year {
                    spans.push(Span::styled(format!("  ({y})"), Style::default().fg(muted)));
                }
                let item = ListItem::new(Line::from(spans));
                if selected { item.style(theme.current_item_style) } else { item }
            })
            .collect();
        let mut state = ListState::default();
        state.select(Some(self.selected));
        frame.render_stateful_widget(List::new(items), inner, &mut state);
    }
}

impl Pane for AlbumsGridPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        if area.width < 6 || area.height < 4 {
            return Ok(());
        }
        // One boxed panel shared by both views (grid and list).
        let accent = ctx.config.theme.highlight_border_style.fg.unwrap_or(Color::Cyan);
        let muted = ctx.config.theme.preview_label_style.fg.unwrap_or(Color::Gray);
        if let Some(panel_bg) = ctx.config.theme.panel_background_color {
            frame.render_widget(Block::default().style(Style::default().bg(panel_bg)), area);
        }
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(ctx.config.as_focused_border_style())
            .title(" \u{f001} Albums ")
            .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
            // design: `N albums` footer chip on the panel border, top-right
            .title(
                Line::styled(format!(" {} albums ", self.albums.len()), Style::default().fg(muted))
                    .right_aligned(),
            );
        let inner = block.inner(area);
        frame.render_widget(block, area);
        self.area = inner;
        if self.view == AlbumsView::List {
            self.render_list(frame, inner, ctx);
            return Ok(());
        }
        let area = inner;
        let theme = &ctx.config.theme;
        let (pad_x, pad_y) = (1u16, 1u16);
        let inner_w = area.width.saturating_sub(pad_x * 2);
        let inner_h = area.height.saturating_sub(pad_y * 2);
        let cols = usize::from((inner_w + GAP_X) / (CARD_W + GAP_X)).max(1);
        let rows_visible = usize::from((inner_h + 1) / CARD_H).max(1);
        self.cols = cols;
        self.rows_visible = rows_visible;

        let faint = theme.text_color.unwrap_or_default();
        if self.albums.is_empty() {
            let msg = "No albums in the library";
            let x = area.x + area.width.saturating_sub(msg.len() as u16) / 2;
            frame.buffer_mut().set_string(
                x,
                area.y + area.height / 2,
                msg,
                Style::default().fg(faint),
            );
            return Ok(());
        }

        // keep the selected card in view
        let sel_row = self.selected / cols;
        if sel_row < self.scroll_row {
            self.scroll_row = sel_row;
        } else if sel_row >= self.scroll_row + rows_visible {
            self.scroll_row = sel_row + 1 - rows_visible;
        }

        let normal = Style::default().fg(theme.text_color.unwrap_or_default());
        let sel_style = theme.current_item_style;
        let buf = frame.buffer_mut();

        for (i, entry) in self.albums.iter().enumerate() {
            let row = i / cols;
            let col = i % cols;
            if row < self.scroll_row || row >= self.scroll_row + rows_visible {
                continue;
            }
            let cx = area.x + pad_x + (col as u16) * (CARD_W + GAP_X);
            let cy = area.y + pad_y + ((row - self.scroll_row) as u16) * CARD_H;

            let selected = i == self.selected;
            let area_bottom = area.y.saturating_add(area.height);
            if cy >= area_bottom {
                continue;
            }
            let cover_h = COVER_H.min(area_bottom - cy);
            let cover_area = Rect::new(cx, cy, CARD_W, cover_h);
            // Selected card: accentSoft fill behind cover and labels with a
            // one-column side bleed — the design's selected-card chip.
            if selected {
                let card_bg = sel_style.bg.unwrap_or(Color::DarkGray);
                let card_bottom = (cy + COVER_H + 3).min(area_bottom);
                let x_end = (cx + CARD_W + 1).min(area.x + area.width);
                for yy in cy..card_bottom {
                    for xx in cx.saturating_sub(1)..x_end {
                        if let Some(cell) = buf.cell_mut((xx, yy)) {
                            cell.set_style(Style::default().bg(card_bg));
                        }
                    }
                }
            }
            if selected && self.crisp && self.has_cover {
                // leave blank — the facade paints the crisp cover here; the buffer
                // cells stay unchanged frame-to-frame so the image persists
                let blank = Style::default().bg(theme
                    .panel_background_color
                    .or(theme.background_color)
                    .unwrap_or_default());
                for yy in 0..cover_h {
                    for xx in 0..CARD_W {
                        if let Some(cell) = buf.cell_mut((cx + xx, cy + yy)) {
                            cell.set_char(' ');
                            cell.set_style(blank);
                        }
                    }
                }
            } else if let Some(img) = self.covers.get(&entry.name) {
                paint_image_cover(buf, cover_area, img);
            } else {
                paint_cover(buf, cover_area, seed_of(&entry.name));
            }

            let label_y = cy + COVER_H;
            // truncate to the card width with a trailing ellipsis (design:
            // `text-overflow: ellipsis` on all three label lines)
            let fit = |s: &str| -> String {
                if s.chars().count() <= CARD_W as usize {
                    s.to_string()
                } else {
                    s.chars()
                        .take((CARD_W as usize).saturating_sub(1))
                        .chain(std::iter::once('…'))
                        .collect()
                }
            };
            // Line 1: album title — semibold (bold+bg when selected)
            if label_y < area_bottom {
                let style = if selected { sel_style } else { normal };
                for (k, ch) in fit(&entry.name).chars().enumerate() {
                    if let Some(cell) = buf.cell_mut((cx + k as u16, label_y)) {
                        cell.set_char(ch);
                        cell.set_style(style);
                    }
                }
            }
            // Line 2: artist — muted (design textMuted)
            let artist_y = label_y + 1;
            if artist_y < area_bottom
                && let Some(artist) = &entry.artist
            {
                for (k, ch) in fit(artist).chars().enumerate() {
                    if let Some(cell) = buf.cell_mut((cx + k as u16, artist_y)) {
                        cell.set_char(ch);
                        cell.set_style(Style::default().fg(muted));
                    }
                }
            }
            // Line 3: `year · genre` — faint (design textFaint)
            let meta_y = artist_y + 1;
            let meta = match (&entry.year, &entry.genre) {
                (Some(y), Some(g)) => {
                    Some(format!("{} · {g}", y.chars().take(4).collect::<String>()))
                }
                (Some(y), None) => Some(y.chars().take(4).collect::<String>()),
                (None, Some(g)) => Some(g.clone()),
                (None, None) => None,
            };
            if meta_y < area_bottom
                && let Some(meta) = meta
            {
                let faint_style = Style::default().fg(muted).add_modifier(Modifier::DIM);
                for (k, ch) in fit(&meta).chars().enumerate() {
                    if let Some(cell) = buf.cell_mut((cx + k as u16, meta_y)) {
                        cell.set_char(ch);
                        cell.set_style(faint_style);
                    }
                }
            }
        }
        if self.crisp
            && let Some(rect) = self.selected_cover_rect()
        {
            self.album_art.set_size(rect);
        }
        Ok(())
    }

    fn before_show(&mut self, ctx: &Ctx) -> Result<()> {
        if !self.initialized {
            ctx.query().id(INIT).replace_id(INIT).target(PaneType::AlbumsGrid).query(
                move |client| {
                    let result = client.list_tag(Tag::Album, None).context("Cannot list albums")?;
                    Ok(MpdQueryResult::LsInfo { data: result.0, path: None })
                },
            );
            self.initialized = true;
        }
        self.fetch_selected_cover(ctx);
        Ok(())
    }

    fn on_query_finished(
        &mut self,
        id: &'static str,
        data: MpdQueryResult,
        _is_visible: bool,
        ctx: &Ctx,
    ) -> Result<()> {
        match (id, data) {
            (INIT, MpdQueryResult::LsInfo { data, .. }) => {
                self.albums = data
                    .into_iter()
                    .map(|name| AlbumEntry { name, artist: None, year: None, genre: None })
                    .collect();
                if self.selected >= self.albums.len() {
                    self.selected = self.albums.len().saturating_sub(1);
                }
                self.fetch_covers(ctx);
                self.fetch_selected_cover(ctx);
                ctx.render()?;
            }
            (COVERS, MpdQueryResult::Any(any)) => {
                if let Ok(covers) = any.downcast::<Vec<(AlbumEntry, Option<RgbaImage>)>>() {
                    // Merge the fetched metadata back into the album list — the
                    // design's cards show artist and `year · genre` under the
                    // title (previously this was fetched and then discarded).
                    let mut meta: HashMap<String, AlbumEntry> =
                        covers.iter().map(|(e, _)| (e.name.clone(), e.clone())).collect();
                    for album in &mut self.albums {
                        if let Some(entry) = meta.remove(&album.name) {
                            album.artist = entry.artist;
                            album.year = entry.year;
                            album.genre = entry.genre;
                        }
                    }
                    self.covers = covers
                        .into_iter()
                        .filter_map(|(entry, img)| img.map(|i| (entry.name, i)))
                        .collect();
                    ctx.render()?;
                }
            }
            (SEL_COVER, MpdQueryResult::AlbumArt(Some(bytes))) => {
                self.has_cover = true;
                self.album_art.show(bytes, ctx)?;
            }
            (SEL_COVER, MpdQueryResult::AlbumArt(None)) => {
                self.has_cover = false;
                self.album_art.hide(ctx)?;
                ctx.render()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_event(&mut self, event: &mut UiEvent, is_visible: bool, ctx: &Ctx) -> Result<()> {
        match event {
            UiEvent::ImageEncoded { data } if is_visible => {
                self.album_art.display(std::mem::take(data), ctx)?;
            }
            UiEvent::Displayed if is_visible && self.crisp && self.has_cover => {
                self.album_art.show_current(ctx)?;
            }
            UiEvent::Exit => {
                self.album_art.cleanup()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_hide(&mut self, ctx: &Ctx) -> Result<()> {
        self.shown_album = None;
        self.has_cover = false;
        self.album_art.hide(ctx)
    }

    fn resize(&mut self, _area: Rect, ctx: &Ctx) -> Result<()> {
        // the grid reflows automatically in render (cols/rows derive from the
        // area); force the crisp cover to re-render at the new geometry so the
        // stale image is cleared and re-placed.
        if self.crisp {
            self.shown_album = None;
            self.has_cover = false;
            self.album_art.hide(ctx)?;
            self.fetch_selected_cover(ctx);
        }
        ctx.render()?;
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick => {
                if let Some(idx) = self.card_at(event.x, event.y) {
                    self.selected = idx;
                    self.fetch_selected_cover(ctx);
                    ctx.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                if let Some(idx) = self.card_at(event.x, event.y) {
                    self.selected = idx;
                    self.fetch_selected_cover(ctx);
                    self.enqueue_selected(ctx);
                    ctx.render()?;
                }
            }
            MouseEventKind::RightClick => {
                if let Some(idx) = self.card_at(event.x, event.y) {
                    self.selected = idx;
                    self.fetch_selected_cover(ctx);
                    self.open_context_menu(ctx);
                }
            }
            MouseEventKind::ScrollDown => {
                let len = self.albums.len();
                if len > 0 {
                    self.selected = (self.selected + self.cols.max(1)).min(len - 1);
                    self.fetch_selected_cover(ctx);
                    ctx.render()?;
                }
            }
            MouseEventKind::ScrollUp if !self.albums.is_empty() => {
                self.selected = self.selected.saturating_sub(self.cols.max(1));
                self.fetch_selected_cover(ctx);
                ctx.render()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_action(&mut self, event: &mut ActionEvent, ctx: &mut Ctx) -> Result<()> {
        let Some(action) = event.claim_common().map(|v| v.to_owned()) else {
            return Ok(());
        };
        let len = self.albums.len();
        if len == 0 {
            return Ok(());
        }
        let cols = self.cols.max(1);
        let page = cols * self.rows_visible.max(1);
        match action {
            CommonAction::Left => self.selected = self.selected.saturating_sub(1),
            CommonAction::Right => self.selected = (self.selected + 1).min(len - 1),
            CommonAction::Up => self.selected = self.selected.saturating_sub(cols),
            CommonAction::Down => self.selected = (self.selected + cols).min(len - 1),
            CommonAction::PageUp => self.selected = self.selected.saturating_sub(page),
            CommonAction::PageDown => self.selected = (self.selected + page).min(len - 1),
            CommonAction::Top => self.selected = 0,
            CommonAction::Bottom => self.selected = len - 1,
            CommonAction::Confirm => self.enqueue_selected(ctx),
            CommonAction::ContextMenu => {
                self.open_context_menu(ctx);
                return Ok(());
            }
            CommonAction::Select => {
                self.view =
                    if self.view == AlbumsView::Grid { AlbumsView::List } else { AlbumsView::Grid };
            }
            _ => return Ok(()),
        }
        self.fetch_selected_cover(ctx);
        ctx.render()?;
        Ok(())
    }
}

/// Paint a decoded cover thumbnail into `area` as upper-half-block cells
/// (fg = top sub-pixel, bg = bottom), the same sub-cell technique as the
/// gradient covers but driven by real image pixels.
fn paint_image_cover(buf: &mut Buffer, area: Rect, img: &RgbaImage) {
    let (iw, ih) = (img.width(), img.height());
    if iw == 0 || ih == 0 {
        return;
    }
    for ry in 0..area.height {
        for cx in 0..area.width {
            let px = u32::from(cx).min(iw - 1);
            let top = img.get_pixel(px, u32::from(ry * 2).min(ih - 1));
            let bot = img.get_pixel(px, u32::from(ry * 2 + 1).min(ih - 1));
            if let Some(cell) = buf.cell_mut((area.x + cx, area.y + ry)) {
                cell.set_symbol("\u{2580}");
                cell.set_fg(Color::Rgb(top[0], top[1], top[2]));
                cell.set_bg(Color::Rgb(bot[0], bot[1], bot[2]));
            }
        }
    }
}
