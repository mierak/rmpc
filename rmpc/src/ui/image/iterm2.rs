use std::io::Write;

use anyhow::{Result, bail};
use base64::Engine;
use ratatui::layout::Rect;

use super::Backend;
use crate::{
    config::{
        Size,
        album_art::{HorizontalAlign, VerticalAlign},
    },
    ctx::Ctx,
    shared::{
        image::{create_aligned_area, get_gif_frames, jpg_encode, resize_image},
        tmux::{self, tmux_write},
    },
    try_skip,
    ui::image::clear_area,
};

#[derive(derive_more::Debug)]
pub struct EncodedData {
    #[debug(skip)]
    content: String,
    aligned_area: Rect,
    size: usize,
    img_width_px: u32,
    img_height_px: u32,
}

#[derive(Debug)]
pub struct Iterm2;

impl Backend for Iterm2 {
    type EncodedData = EncodedData;

    fn hide(
        &mut self,
        w: &mut impl Write,
        size: Rect,
        bg_color: Option<crossterm::style::Color>,
    ) -> Result<()> {
        clear_area(w, bg_color, size)
    }

    fn display(&mut self, w: &mut impl Write, data: Self::EncodedData, _ctx: &Ctx) -> Result<()> {
        try_skip!(display(w, data), "Failed to display iterm2 image");
        Ok(())
    }

    fn create_data(
        image_data: &[u8],
        area: Rect,
        max_size: Size,
        halign: HorizontalAlign,
        valign: VerticalAlign,
    ) -> Result<Self::EncodedData> {
        let start = std::time::Instant::now();

        let (len, width_px, height_px, data, aligned_area) = if let Some(gif_data) =
            get_gif_frames(image_data)?
        {
            log::debug!("encoding animated gif");

            // Take smaller of the two dimensions to make the gif stretch over available
            // area and not overflow
            let (width, height) = gif_data.dimensions;
            let aligned_area = create_aligned_area(area, (width, height), max_size, halign, valign);
            log::debug!(aligned_area:?, dims:? = gif_data.dimensions; "encoded");

            let size = aligned_area.size_px.width.min(aligned_area.size_px.height).into();

            (
                image_data.len(),
                size,
                size,
                base64::engine::general_purpose::STANDARD.encode(image_data),
                aligned_area,
            )
        } else {
            let (image, aligned_area) =
                match resize_image(image_data, area, max_size, halign, valign) {
                    Ok(v) => v,
                    Err(err) => {
                        bail!("Failed to resize image, err: {err}");
                    }
                };
            let Ok(jpg) = jpg_encode(&image) else { bail!("Failed to encode image as jpg") };
            (
                jpg.len(),
                image.width(),
                image.height(),
                base64::engine::general_purpose::STANDARD.encode(&jpg),
                aligned_area,
            )
        };

        log::debug!(compressed_bytes = data.len(), image_bytes = len, elapsed:? = start.elapsed(); "encoded data");
        Ok(EncodedData {
            content: data,
            size: len,
            img_width_px: width_px,
            img_height_px: height_px,
            aligned_area: aligned_area.area,
        })
    }
}

fn display(w: &mut impl Write, data: EncodedData) -> Result<()> {
    let EncodedData { content, size, aligned_area, img_width_px, img_height_px } = data;

    // Adjust for tmux pane position if inside tmux
    let (x, y) = if tmux::is_inside_tmux() {
        match tmux::pane_position() {
            Ok(pane_position) => {
                (aligned_area.x + 1 + pane_position.0, aligned_area.y + 1 + pane_position.1)
            }
            Err(err) => {
                log::error!(
                    "Failed to get tmux pane position, falling back to unadjusted position, err: {err}"
                );
                (aligned_area.x + 1, aligned_area.y + 1)
            }
        }
    } else {
        (aligned_area.x + 1, aligned_area.y + 1)
    };

    // TODO: https://iterm2.com/documentation-images.html
    // A new way of sending files was introduced in iTerm2 version 3.5 which
    // works in tmux integration mode by splitting the giant control
    // sequence into a number of smaller ones: First, send:
    // ESC ] 1337 ; MultipartFile = [optional arguments] ^G
    // Then, send one or more of:
    // ESC ] 1337 ; FilePart = base64 encoded file contents ^G
    // What size chunks should you use? Older versions of tmux have a limit of
    // 256 bytes for the entire sequence. In newer versions of tmux, the
    // limit is 1,048,576 bytes. iTerm2 also imposes a limit of 1,048,576
    // bytes. Finally, send: ESC ] 1337 ; FileEnd ^G
    tmux_write!(
        w,
        "\x1B7\x1b[{y};{x}H\x1b]1337;File=inline=1;size={size};width={img_width_px}px;height={img_height_px}px;preserveAspectRatio=1;doNotMoveCursor=1:{content}\x1B\x5C\x1b\n\x1B8",
    )?;

    w.flush()?;

    Ok(())
}
