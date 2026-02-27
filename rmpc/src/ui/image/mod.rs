use std::io::Write;

use anyhow::Result;
use crossterm::{
    cursor::{RestorePosition, SavePosition},
    queue,
    style::{Colors, ResetColor, SetColors},
};
use ratatui::layout::Rect;

use crate::{
    config::{
        Size,
        album_art::{HorizontalAlign, VerticalAlign},
    },
    ctx::Ctx,
    shared::macros::csi_move,
};

pub mod block;
pub mod facade;
pub mod iterm2;
pub mod kitty;
pub mod sixel;
pub mod ueberzug;

#[allow(unused)]
pub trait Backend {
    type EncodedData;
    fn hide(
        &mut self,
        w: &mut impl Write,
        size: Rect,
        bg_color: Option<crossterm::style::Color>,
    ) -> Result<()>;
    fn cleanup(self: Box<Self>, rect: Rect) -> Result<()> {
        Ok(())
    }
    fn display(&mut self, w: &mut impl Write, data: Self::EncodedData, ctx: &Ctx) -> Result<()>;
    fn create_data(
        image_data: &[u8],
        area: Rect,
        max_size: Size,
        halign: HorizontalAlign,
        valign: VerticalAlign,
    ) -> Result<Self::EncodedData>;
}

pub fn clear_area(
    w: &mut impl Write,
    bg_color: Option<crossterm::style::Color>,
    area: Rect,
) -> Result<()> {
    let colors = Colors { background: bg_color, foreground: None };
    queue!(w, SetColors(colors))?;
    queue!(w, SavePosition)?;
    let capacity: usize = 2usize * area.width as usize * area.height as usize;
    let mut buf = Vec::with_capacity(capacity);
    for y in area.top()..area.bottom() {
        csi_move!(buf, area.x, y)?;
        for _ in 0..area.width {
            write!(buf, " ")?;
        }
    }

    w.write_all(&buf)?;
    w.flush()?;
    queue!(w, RestorePosition)?;
    queue!(w, ResetColor)?;

    Ok(())
}
