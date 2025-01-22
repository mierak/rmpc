use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use anyhow::{Result, bail};
use base64::Engine;
use crossbeam::channel::{Sender, unbounded};
use crossterm::cursor::{MoveTo, RestorePosition, SavePosition};
use crossterm::queue;
use crossterm::style::Colors;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::Backend;
use crate::config::Size;
use crate::config::album_art::{HorizontalAlign, VerticalAlign};
use crate::shared::ext::mpsc::RecvLast;
use crate::shared::image::{create_aligned_area, get_gif_frames, jpg_encode, resize_image};
use crate::shared::macros::try_cont;
use crate::shared::tmux::tmux_write;
use crate::ui::image::clear_area;
use crate::ui::image::facade::IS_SHOWING;

#[derive(Debug)]
struct EncodedData {
    content: String,
    aligned_area: Rect,
    size: usize,
    img_width_px: u32,
    img_height_px: u32,
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
}

impl Iterm2 {
    pub fn new(
        max_size: Size,
        bg_color: Option<Color>,
        halign: HorizontalAlign,
        valign: VerticalAlign,
    ) -> Self {
        let (sender, receiver) = unbounded::<DataToEncode>();
        let colors = Colors { background: bg_color.map(Into::into), foreground: None };

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
                        encode(area, &data, max_size, halign, valign),
                        "Failed to encode data"
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

                    try_cont!(
                        clear_area(&mut w, colors, area),
                        "Failed to clear iterm2 image area"
                    );
                    try_cont!(display(&mut w, encoded), "Failed to display iterm2 image");
                }
            })
            .expect("iterm2 thread to be spawned");

        Self { sender, colors }
    }
}

fn display(w: &mut impl Write, data: EncodedData) -> Result<()> {
    let EncodedData { content, size, aligned_area, img_width_px, img_height_px } = data;

    queue!(w, SavePosition)?;
    queue!(w, MoveTo(aligned_area.x, aligned_area.y))?;

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
        "\x1b]1337;File=inline=1;size={size};width={img_width_px}px;height={img_height_px}px;preserveAspectRatio=1;doNotMoveCursor=1:{content}\x08\x1b\n"
    )?;
    queue!(w, RestorePosition)?;

    Ok(())
}

fn encode(
    area: Rect,
    data: &[u8],
    max_size_px: Size,
    halign: HorizontalAlign,
    valign: VerticalAlign,
) -> Result<EncodedData> {
    let start = std::time::Instant::now();

    let (len, width_px, height_px, data, aligned_area) = if let Some(gif_data) =
        get_gif_frames(data)?
    {
        log::debug!("encoding animated gif");

        // Take smaller of the two dimensions to make the gif stretch over available
        // area and not overflow
        let (width, height) = gif_data.dimensions;
        let aligned_area = create_aligned_area(area, (width, height), max_size_px, halign, valign);
        log::debug!(aligned_area:?, dims:? = gif_data.dimensions; "encoded");

        let size = aligned_area.size_px.width.min(aligned_area.size_px.height).into();

        (
            data.len(),
            size,
            size,
            base64::engine::general_purpose::STANDARD.encode(data),
            aligned_area,
        )
    } else {
        let (image, aligned_area) = match resize_image(data, area, max_size_px, halign, valign) {
            Ok(v) => v,
            Err(err) => {
                bail!("Failed to resize image, err: {}", err);
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
