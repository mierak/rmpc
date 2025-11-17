use std::{
    io::Write,
    sync::{Arc, atomic::Ordering},
};

use anyhow::{Result, bail};
use base64::Engine;
use crossbeam::channel::{Sender, unbounded};
use crossterm::style::Colors;
use ratatui::layout::Rect;

use super::{AlbumArtConfig, Backend, EncodeRequest, ImageBackendRequest};
use crate::{
    config::{
        Size,
        album_art::{HorizontalAlign, VerticalAlign},
    },
    shared::{
        image::{create_aligned_area, get_gif_frames, jpg_encode, resize_image},
        macros::try_cont,
        terminal::TERMINAL,
        tmux::{self, tmux_write},
    },
    try_skip,
    ui::image::{clear_area, facade::IS_SHOWING, recv_data},
};

#[derive(Debug)]
struct EncodedData {
    content: String,
    aligned_area: Rect,
    size: usize,
    img_width_px: u32,
    img_height_px: u32,
}

#[derive(Debug)]
pub struct Iterm2 {
    sender: Sender<ImageBackendRequest>,
    colors: Colors,
    handle: std::thread::JoinHandle<()>,
}

impl Backend for Iterm2 {
    fn hide(&mut self, size: Rect) -> Result<()> {
        let writer = TERMINAL.writer();
        let mut writer = writer.lock();
        clear_area(writer.by_ref(), self.colors, size)
    }

    fn show(&mut self, data: Arc<Vec<u8>>, area: Rect) -> Result<()> {
        Ok(self.sender.send(ImageBackendRequest::Encode(EncodeRequest { area, data }))?)
    }

    fn set_config(&self, config: AlbumArtConfig) -> Result<()> {
        Ok(self.sender.send(ImageBackendRequest::SetConfig(config))?)
    }

    fn cleanup(self: Box<Self>, _area: Rect) -> Result<()> {
        self.sender.send(ImageBackendRequest::Stop)?;
        self.handle.join().expect("iterm2 thread to end gracefully");
        Ok(())
    }
}

impl Iterm2 {
    pub(super) fn new(config: AlbumArtConfig) -> Self {
        let (sender, receiver) = unbounded::<ImageBackendRequest>();
        let colors = config.colors;

        let handle = std::thread::Builder::new()
            .name("iterm2".to_string())
            .spawn(move || {
                let mut config = config;
                let mut pending_req = None;
                'outer: loop {
                    let EncodeRequest { data, area } =
                        match recv_data(&mut pending_req, &mut config, &receiver) {
                            Ok(Some(msg)) => msg,
                            Ok(None) => break,
                            Err(err) => {
                                log::error!("Error receiving ImageBackendRequest message: {err}");
                                break;
                            }
                        };

                    let encoded = try_cont!(
                        encode(area, &data, config.max_size, config.halign, config.valign),
                        "Failed to encode data"
                    );

                    // consume all pending messages, skipping older encode requests
                    for msg in receiver.try_iter() {
                        match msg {
                            ImageBackendRequest::Stop => break 'outer,
                            ImageBackendRequest::SetConfig(cfg) => config = cfg,
                            ImageBackendRequest::Encode(req) => {
                                pending_req = Some(req);
                                log::debug!(
                                    "Skipping image because another one is waiting in the queue"
                                );
                                continue 'outer;
                            }
                        }
                    }

                    let writer = TERMINAL.writer();
                    let mut writer = writer.lock();
                    let mut w = writer.by_ref();
                    if !IS_SHOWING.load(Ordering::Relaxed) {
                        log::trace!(
                            "Not showing image because its not supposed to be displayed anymore"
                        );
                        continue;
                    }

                    try_cont!(
                        clear_area(&mut w, config.colors, area),
                        "Failed to clear iterm2 image area"
                    );
                    try_skip!(display(&mut w, encoded), "Failed to display iterm2 image");
                }
            })
            .expect("iterm2 thread to be spawned");

        Self { sender, colors, handle }
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
        "\x1B7\x1b[{y};{x}H\x1b]1337;File=inline=1;size={size};width={img_width_px}px;height={img_height_px}px;preserveAspectRatio=1;doNotMoveCursor=1:{content}\x08\x1b\n\x1B8",
    )?;

    w.flush()?;

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
