use std::{io::Write, sync::Arc};

use anyhow::Result;
use crossbeam::channel::Receiver;
use crossterm::{
    cursor::{RestorePosition, SavePosition},
    queue,
    style::{Colors, SetColors},
};
use ratatui::layout::Rect;

use crate::{
    config::{
        Config,
        Size,
        album_art::{HorizontalAlign, VerticalAlign},
    },
    shared::macros::csi_move,
};

pub mod facade;
pub mod iterm2;
pub mod kitty;
pub mod sixel;
pub mod ueberzug;

#[allow(unused)]
trait Backend {
    fn hide(&mut self, size: Rect) -> Result<()>;
    fn show(&mut self, data: Arc<Vec<u8>>, area: Rect) -> Result<()>;
    fn cleanup(self: Box<Self>, rect: Rect) -> Result<()> {
        Ok(())
    }
    fn set_config(&self, config: AlbumArtConfig) -> Result<()>;
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

#[derive(Debug)]
struct AlbumArtConfig {
    max_size: Size,
    colors: Colors,
    valign: VerticalAlign,
    halign: HorizontalAlign,
}
impl From<&Config> for AlbumArtConfig {
    fn from(config: &Config) -> Self {
        Self {
            max_size: config.album_art.max_size_px,
            colors: Colors {
                background: config.theme.background_color.map(Into::into),
                foreground: None,
            },
            valign: config.album_art.vertical_align,
            halign: config.album_art.horizontal_align,
        }
    }
}

#[derive(Debug)]
enum ImageBackendRequest {
    Stop,
    Encode(EncodeRequest),
    SetConfig(AlbumArtConfig),
}

#[derive(Debug)]
struct EncodeRequest {
    area: Rect,
    data: Arc<Vec<u8>>,
}

fn recv_data(
    pending_req: &mut Option<EncodeRequest>,
    config: &mut AlbumArtConfig,
    rx: &Receiver<ImageBackendRequest>,
) -> Result<Option<EncodeRequest>> {
    let mut message = pending_req.take();

    // consume all pending messages, skipping older encode requests
    for msg in rx.try_iter() {
        match msg {
            ImageBackendRequest::Stop => return Ok(None),
            ImageBackendRequest::SetConfig(album_art_config) => *config = album_art_config,
            ImageBackendRequest::Encode(encode_request) => message = Some(encode_request),
        }
    }

    if let Some(message) = message {
        return Ok(Some(message));
    }

    loop {
        match rx.recv() {
            Ok(msg) => match msg {
                ImageBackendRequest::Stop => return Ok(None),
                ImageBackendRequest::SetConfig(album_art_config) => *config = album_art_config,
                ImageBackendRequest::Encode(encode_request) => return Ok(Some(encode_request)),
            },
            Err(err) => {
                return Err(err.into());
            }
        }
    }
}
