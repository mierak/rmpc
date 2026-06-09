use anyhow::{Context, Result};
use ratatui::{Frame, layout::Rect, style::Style};
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
        mpd_query::MpdQueryResult,
    },
};

const INIT: &str = "albums_grid_init";

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

            paint_cover(buf, Rect::new(cx, cy, CARD_W, COVER_H), seed_of(name));

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
        if let (INIT, MpdQueryResult::LsInfo { data, .. }) = (id, data) {
            self.albums = data;
            if self.selected >= self.albums.len() {
                self.selected = self.albums.len().saturating_sub(1);
            }
            ctx.render()?;
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
