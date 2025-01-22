use std::{io::Write, sync::Arc};

use anyhow::Result;
use crossterm::{
    cursor::{RestorePosition, SavePosition},
    queue,
    style::{Colors, SetColors},
};
use ratatui::layout::Rect;

use crate::shared::macros::csi_move;

pub mod facade;
pub mod iterm2;
pub mod kitty;
pub mod sixel;
pub mod ueberzug;

#[allow(unused)]
pub trait Backend {
    fn hide(&mut self, size: Rect) -> Result<()>;
    fn show(&mut self, data: Arc<Vec<u8>>, area: Rect) -> Result<()>;
    fn cleanup(self: Box<Self>, rect: Rect) -> Result<()> {
        Ok(())
    }
}

pub fn clear_area(w: &mut impl Write, colors: Colors, area: Rect) -> Result<()> {
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

    Ok(())
}
