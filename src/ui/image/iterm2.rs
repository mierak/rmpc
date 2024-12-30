use anyhow::{bail, Result};
use base64::Engine;
use crossbeam::channel::{unbounded, Sender};
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    queue,
    style::Colors,
};
use std::{
    io::Write,
    sync::{atomic::Ordering, Arc},
};

use ratatui::{layout::Rect, style::Color};

use crate::{
    config::Size,
    shared::{
        ext::mpsc::RecvLast,
        image::{get_gif_frames, get_image_area_size_px, jpg_encode, resize_image},
        macros::try_cont,
        tmux::tmux_write,
    },
    ui::image::{clear_area, facade::IS_SHOWING},
};

use super::Backend;

#[derive(Debug)]
struct EncodedData {
    content: String,
    size: usize,
    width: u32,
    height: u32,
}

#[derive(Debug)]
struct DataToEncode {
    area: Rect,
    data: Arc<Vec<u8>>,
}

#[derive(Debug)]
pub struct Iterm2 {
    sender: Sender<DataToEncode>,
    colors: Colors,
}

impl Backend for Iterm2 {
    fn hide(&mut self, size: Rect) -> Result<()> {
        clear_area(&mut std::io::stdout().lock(), self.colors, size)
    }

    fn show(&mut self, data: Arc<Vec<u8>>, area: Rect) -> Result<()> {
        Ok(self.sender.send(DataToEncode { area, data })?)
    }

    fn resize(&mut self) {}
}

impl Iterm2 {
    pub fn new(max_size: Size, bg_color: Option<Color>) -> Self {
        let (sender, receiver) = unbounded::<DataToEncode>();
        let colors = Colors {
            background: bg_color.map(Into::into),
            foreground: None,
        };

        std::thread::Builder::new()
            .name("iterm2".to_string())
            .spawn(move || {
                let mut pending_req = None;
                loop {
                    let Ok(DataToEncode { area, data }) =
                        pending_req.take().ok_or(()).or_else(|()| receiver.recv_last())
                    else {
                        continue;
                    };

                    let encoded = try_cont!(
                        encode(area.width, area.height, &data, max_size),
                        "Failed to encode data"
                    );

                    let mut w = std::io::stdout().lock();
                    if !IS_SHOWING.load(Ordering::Relaxed) {
                        log::trace!("Not showing image because its not supposed to be displayed anymore");
                        continue;
                    }

                    if let Ok(msg) = receiver.try_recv_last() {
                        pending_req = Some(msg);
                        log::trace!("Skipping image because another one is waiting in the queue");
                        continue;
                    };

                    try_cont!(clear_area(&mut w, colors, area), "Failed to clear iterm2 image area");
                    try_cont!(display(&mut w, encoded, area), "Failed to display iterm2 image");
                }
            })
            .expect("iterm2 thread to be spawned");

        Self { sender, colors }
    }
}

fn display(w: &mut impl Write, data: EncodedData, area: Rect) -> Result<()> {
    let EncodedData {
        content,
        size,
        width,
        height,
    } = data;

    queue!(w, SavePosition)?;
    queue!(w, MoveTo(area.x, area.y))?;

    tmux_write!(w, "\x1b]1337;File=inline=1;size={size};width={width}px;height={height}px;preserveAspectRatio=1;doNotMoveCursor=1:{content}\x07")?;
    queue!(w, RestorePosition)?;

    Ok(())
}

fn encode(width: u16, height: u16, data: &[u8], max_size_px: Size) -> Result<EncodedData> {
    let start = std::time::Instant::now();
    let (iwidth, iheight) = match get_image_area_size_px(width, height, max_size_px) {
        Ok(v) => v,
        Err(err) => {
            bail!("Failed to get image size, err: {}", err);
        }
    };

    let (len, data) = if get_gif_frames(data)?.is_some() {
        log::debug!("encoding animated gif");
        (data.len(), base64::engine::general_purpose::STANDARD.encode(data))
    } else {
        let image = match resize_image(data, iwidth, iheight) {
            Ok(v) => v,
            Err(err) => {
                bail!("Failed to resize image, err: {}", err);
            }
        };
        let Ok(jpg) = jpg_encode(&image) else {
            bail!("Failed to encode image as jpg")
        };
        (jpg.len(), base64::engine::general_purpose::STANDARD.encode(&jpg))
    };

    log::debug!(compressed_bytes = data.len(), image_bytes = len, elapsed:? = start.elapsed(); "encoded data");
    Ok(EncodedData {
        content: data,
        size: len,
        width: u32::from(iwidth),
        height: u32::from(iheight),
    })
}
