use std::{collections::VecDeque, io::Write, ops::AddAssign, time::Instant};

use anyhow::{Result, bail};
use color_quant::NeuQuant;
use image::Rgba;
use ratatui::layout::Rect;

use super::{Backend, clear_area};
use crate::{
    config::{
        Size,
        album_art::{HorizontalAlign, VerticalAlign},
    },
    ctx::Ctx,
    shared::{image::resize_image, tmux::tmux_write_bytes},
    tmux,
};

#[derive(Debug)]
pub struct Sixel;

#[derive(derive_more::Debug)]
pub struct Data {
    #[debug(skip)]
    content: VecDeque<u8>,
    area: Rect,
}

impl Backend for Sixel {
    type EncodedData = Data;

    fn hide(
        &mut self,
        w: &mut impl Write,
        size: Rect,
        bg_color: Option<crossterm::style::Color>,
    ) -> anyhow::Result<()> {
        clear_area(w, bg_color, size)
    }

    fn display(
        &mut self,
        w: &mut impl Write,
        mut data: Self::EncodedData,
        _ctx: &Ctx,
    ) -> Result<()> {
        log::debug!(bytes = data.content.len(); "transmitting data");

        // Adjust for tmux pane position if inside tmux
        let (x, y) = if tmux::is_inside_tmux() {
            match tmux::pane_position() {
                Ok(pane_position) => {
                    (data.area.x + 1 + pane_position.0, data.area.y + 1 + pane_position.1)
                }
                Err(err) => {
                    log::error!(
                        "Failed to get tmux pane position, falling back to unadjusted position, err: {err}"
                    );
                    (data.area.x + 1, data.area.y + 1)
                }
            }
        } else {
            (data.area.x + 1, data.area.y + 1)
        };

        for b in format!("\x1b7\x1b[{y};{x}H").as_bytes().iter().rev() {
            data.content.push_front(*b);
        }

        for b in "\x1b8".as_bytes() {
            data.content.push_back(*b);
        }

        tmux_write_bytes!(w, data.content.make_contiguous());
        w.flush()?;

        Ok(())
    }

    fn create_data(
        image_data: &[u8],
        area: Rect,
        max_size: Size,
        halign: HorizontalAlign,
        valign: VerticalAlign,
    ) -> Result<Self::EncodedData> {
        let start = Instant::now();

        let (image, resized_area) = match resize_image(image_data, area, max_size, halign, valign) {
            Ok(v) => v,
            Err(err) => {
                bail!("Failed to resize image, err: {err}");
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
                bail!(
                    "Tmux supports a maximum of 1MB of data. Sixel image will not be displayed. Try decreasing max album art size."
                )
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
        Ok(Data { content: VecDeque::from(buf), area: resized_area.area })
    }
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
