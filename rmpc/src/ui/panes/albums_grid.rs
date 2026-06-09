use std::collections::HashMap;

use anyhow::{Context, Result};
use image::RgbaImage;
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
};
use rmpc_mpd::{
    filter::{Filter, Tag},
    mpd_client::MpdClient,
};

use super::{
    Pane,
    gradient_art::{paint_cover, seed_of},
};
use crate::{
    config::{keys::CommonAction, tabs::PaneType},
    ctx::Ctx,
    shared::{
        keys::ActionEvent,
        mouse_event::{MouseEvent, MouseEventKind},
        mpd_client_ext::MpdClientExt,
        mpd_query::MpdQueryResult,
    },
};

const INIT: &str = "albums_grid_init";
const COVERS: &str = "albums_grid_covers";

// card geometry (cells)
const CARD_W: u16 = 18;
const GAP_X: u16 = 2;
const COVER_H: u16 = 8;
const CARD_H: u16 = COVER_H + 2; // cover + title row + spacing row

/// A responsive grid of generative album covers — the design's Albums screen.
///
/// Each album gets a procedural cover painted with [`paint_cover`] (sub-cell
/// half-block gradient, seeded by the album name), laid out in a grid that
/// reflows to the pane width. Arrow keys / hjkl move the selection, Enter
/// enqueues the album.
#[derive(Debug, Default)]
pub struct AlbumsGridPane {
    albums: Vec<String>,
    selected: usize,
    scroll_row: usize,
    cols: usize,
    rows_visible: usize,
    area: Rect,
    covers: HashMap<String, RgbaImage>,
    initialized: bool,
}

impl AlbumsGridPane {
    pub fn new() -> Self {
        Self::default()
    }

    fn enqueue_selected(&self, ctx: &Ctx) {
        let Some(name) = self.albums.get(self.selected).cloned() else {
            return;
        };
        ctx.command(move |_, client| {
            client
                .find_add(&[Filter::new(Tag::Album, name.as_str())], None)
                .context("Failed to add album to queue")?;
            Ok(())
        });
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
                let mut out: Vec<(String, Option<RgbaImage>)> = Vec::with_capacity(albums.len());
                for album in albums {
                    let cover = (|| -> Result<Option<RgbaImage>> {
                        let Some(song) =
                            client.find_one(&[Filter::new(Tag::Album, album.as_str())])?
                        else {
                            return Ok(None);
                        };
                        let Some(bytes) = client.find_album_art(&song.file, order)? else {
                            return Ok(None);
                        };
                        let img = image::load_from_memory(&bytes)?.to_rgba8();
                        Ok(Some(image::imageops::resize(
                            &img,
                            tw,
                            th,
                            image::imageops::FilterType::Lanczos3,
                        )))
                    })()
                    .unwrap_or(None);
                    out.push((album, cover));
                }
                Ok(MpdQueryResult::Any(Box::new(out)))
            },
        );
    }
}

impl Pane for AlbumsGridPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        self.area = area;
        if area.width < 6 || area.height < 4 {
            return Ok(());
        }
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

        for (i, name) in self.albums.iter().enumerate() {
            let row = i / cols;
            let col = i % cols;
            if row < self.scroll_row || row >= self.scroll_row + rows_visible {
                continue;
            }
            let cx = area.x + pad_x + (col as u16) * (CARD_W + GAP_X);
            let cy = area.y + pad_y + ((row - self.scroll_row) as u16) * CARD_H;

            let cover_area = Rect::new(cx, cy, CARD_W, COVER_H);
            if let Some(img) = self.covers.get(name) {
                paint_image_cover(buf, cover_area, img);
            } else {
                paint_cover(buf, cover_area, seed_of(name));
            }

            let selected = i == self.selected;
            let style = if selected { sel_style } else { normal };
            // pad the label to the card width so the selection highlight reads as a bar
            let mut label: String = name.chars().take(CARD_W as usize).collect();
            let w = label.chars().count();
            if w < CARD_W as usize {
                label.push_str(&" ".repeat(CARD_W as usize - w));
            }
            buf.set_string(cx, cy + COVER_H, &label, style);
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
                self.albums = data;
                if self.selected >= self.albums.len() {
                    self.selected = self.albums.len().saturating_sub(1);
                }
                self.fetch_covers(ctx);
                ctx.render()?;
            }
            (COVERS, MpdQueryResult::Any(any)) => {
                if let Ok(covers) = any.downcast::<Vec<(String, Option<RgbaImage>)>>() {
                    self.covers = covers
                        .into_iter()
                        .filter_map(|(name, img)| img.map(|i| (name, i)))
                        .collect();
                    ctx.render()?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_mouse_event(&mut self, event: MouseEvent, ctx: &Ctx) -> Result<()> {
        match event.kind {
            MouseEventKind::LeftClick => {
                if let Some(idx) = self.card_at(event.x, event.y) {
                    self.selected = idx;
                    ctx.render()?;
                }
            }
            MouseEventKind::DoubleClick => {
                if let Some(idx) = self.card_at(event.x, event.y) {
                    self.selected = idx;
                    self.enqueue_selected(ctx);
                    ctx.render()?;
                }
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
            CommonAction::Down => {
                if self.selected + cols < len {
                    self.selected += cols;
                }
            }
            CommonAction::PageUp => self.selected = self.selected.saturating_sub(page),
            CommonAction::PageDown => self.selected = (self.selected + page).min(len - 1),
            CommonAction::Top => self.selected = 0,
            CommonAction::Bottom => self.selected = len - 1,
            CommonAction::Confirm => self.enqueue_selected(ctx),
            _ => return Ok(()),
        }
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
