use std::{
    io::Write,
    ops::AddAssign,
    sync::{Arc, atomic::Ordering},
    time::Instant,
};

use anyhow::{Result, bail};
use color_quant::NeuQuant;
use crossbeam::channel::{Sender, unbounded};
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    queue,
    style::Colors,
};
use image::Rgba;
use ratatui::{layout::Rect, style::Color};

use super::{Backend, clear_area};
use crate::{
    config::{
        Size,
        album_art::{HorizontalAlign, VerticalAlign},
    },
    shared::{
        ext::mpsc::RecvLast,
        image::resize_image,
        macros::{status_error, try_cont},
    },
    tmux,
    ui::image::facade::IS_SHOWING,
};

#[derive(Debug)]
pub struct Sixel {
    sender: Sender<DataToEncode>,
    colors: Colors,
}

#[derive(Debug)]
struct DataToEncode {
    area: Rect,
    data: Arc<Vec<u8>>,
}

impl Backend for Sixel {
    fn hide(&mut self, size: Rect) -> anyhow::Result<()> {
        clear_area(&mut std::io::stdout().lock(), self.colors, size)
    }

    fn show(&mut self, data: Arc<Vec<u8>>, area: Rect) -> Result<()> {
        Ok(self.sender.send(DataToEncode { area, data })?)
    }
}

impl Sixel {
    pub fn new(
        max_size: Size,
        bg_color: Option<Color>,
        halign: HorizontalAlign,
        valign: VerticalAlign,
    ) -> Self {
        let (sender, receiver) = unbounded::<DataToEncode>();
        let colors = Colors { background: bg_color.map(Into::into), foreground: None };

        std::thread::Builder::new()
            .name("sixel".to_string())
            .spawn(move || {
                let mut pending_req = None;
                loop {
                    let Ok(DataToEncode { area, data }) =
                        pending_req.take().ok_or(()).or_else(|()| receiver.recv_last())
                    else {
                        continue;
                    };

                    let (buf, resized_area) = try_cont!(
                        encode(&data, area, max_size, halign, valign),
                        "Failed to encode"
                    );

                    let mut w = std::io::stdout().lock();
                    if !IS_SHOWING.load(Ordering::Relaxed) {
                        log::trace!(
                            "Not showing image because its not supposed to be displayed anymore"
                        );
                        continue;
                    }

                    if let Ok(msg) = receiver.try_recv_last() {
                        pending_req = Some(msg);
                        log::trace!("Skipping image because another one is waiting in the queue");
                        continue;
                    };

                    try_cont!(clear_area(&mut w, colors, area), "Failed to clear sixel image area");
                    try_cont!(display(&mut w, &buf, resized_area), "Failed to display sixel image");
                }
            })
            .expect("sixel thread to be spawned");

        Self { sender, colors }
    }
}

fn display(w: &mut impl Write, data: &[u8], area: Rect) -> Result<()> {
    log::debug!(bytes = data.len(); "transmitting data");
    queue!(w, SavePosition)?;
    queue!(w, MoveTo(area.x, area.y))?;
    w.write_all(data)?;
    w.flush()?;
    queue!(w, RestorePosition)?;

    Ok(())
}

fn encode(
    data: &[u8],
    area: Rect,
    max_size: Size,
    halign: HorizontalAlign,
    valign: VerticalAlign,
) -> Result<(Vec<u8>, Rect)> {
    let start = Instant::now();

    let (image, resized_area) = match resize_image(data, area, max_size, halign, valign) {
        Ok(v) => v,
        Err(err) => {
            bail!("Failed to resize image, err: {}", err);
        }
    };

    let width = image.width();
    let height = image.height();
    let tmux = tmux::is_inside_tmux();

    let mut buf = Vec::new();

    if tmux {
        write!(buf, "\x1bPtmux;\x1b\x1bP0;1;7q\"1;1;{};{}", image.width(), image.height())?;
    } else {
        write!(buf, "\x1bP0;1;7q\"1;1;{};{}", image.width(), image.height())?;
    }

    let image = image.to_rgba8();
    let quantized = NeuQuant::new(10, 256, image.as_raw());
    for (i, [r, g, b]) in quantized.color_map_rgb().u16_triples().enumerate() {
        write!(buf, "#{i};2;{r};{g};{b}")?;
    }

    for y in 0..height {
        let character: u8 = 63 + 2u8.pow(y % 6);
        let mut repeat = 0;
        let mut last_color = None;

        for x in 0..width {
            let Rgba(current_pixel) = image.get_pixel(x, y);
            let color = quantized.index_of(current_pixel);

            if last_color.is_some_and(|c| c == color) || last_color.is_none() {
                repeat.add_assign(1);
                last_color = Some(color);
                continue;
            }

            put_color(&mut buf, character, last_color.unwrap_or_default(), repeat)?;

            last_color = Some(color);
            repeat = 1;
        }

        if tmux && buf.len() > 1_048_576 {
            status_error!(
                "Tmux supports a maximum of 1MB of data. Sixel image will not be displayed. Try decreasing max album art size.",
            );
            bail!("Exceeded tmux data limit")
        }

        put_color(&mut buf, character, last_color.unwrap_or_default(), repeat)?;

        buf.push(if y % 6 == 5 { b'-' } else { b'$' });
    }

    if tmux {
        write!(buf, "\x1b\\\x1b\\")?;
    } else {
        write!(buf, "\x1b\\")?;
    }

    log::debug!(bytes = buf.len(), image_bytes = image.len(), elapsed:? = start.elapsed(); "encoded data");
    Ok((buf, resized_area.area))
}

fn put_color<W: Write>(
    buf: &mut W,
    byte: u8,
    color: usize,
    repeat: u16,
) -> Result<(), std::io::Error> {
    if repeat == 0 {
        write!(buf, "#{}{}", color, byte as char)
    } else {
        write!(buf, "#{}!{repeat}{}", color, byte as char)
    }
}

struct U16Triples {
    data: Vec<u8>,
    current: usize,
}

trait IntoU16Triples {
    fn u16_triples(self) -> U16Triples;
}

impl IntoU16Triples for Vec<u8> {
    fn u16_triples(self) -> U16Triples {
        U16Triples { data: self, current: 0 }
    }
}

impl Iterator for U16Triples {
    type Item = [u16; 3];

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.len() - self.current < 3 {
            return None;
        }
        let a = u16::from(self.data[self.current]);
        self.current += 1;
        let b = u16::from(self.data[self.current]);
        self.current += 1;
        let c = u16::from(self.data[self.current]);
        self.current += 1;

        Some([a * 100 / 255, b * 100 / 255, c * 100 / 255])
    }
}
