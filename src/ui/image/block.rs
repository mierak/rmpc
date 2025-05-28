use super::{AlbumArtConfig, Backend, ImageBackendRequest, clear_area};
use crate::shared::ansi_colors::ansi256_from_rgb;
use crate::{
    config::{
        Size,
        album_art::{HorizontalAlign, VerticalAlign},
    },
    shared::{
        image::{AlignedArea, resize_image},
        macros::{status_error, try_cont},
    },
    try_skip,
    ui::image::{EncodeRequest, facade::IS_SHOWING, recv_data},
};
use anyhow::{Result, bail};
use crossbeam::channel::{Sender, unbounded};
use crossterm::style::{ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    queue,
    style::{Color, Colors, Print},
};
use image::{DynamicImage, GenericImageView, Rgba};
use ratatui::layout::Rect;
use std::io::Write;
use std::sync::{Arc, atomic::Ordering};

#[derive(Debug)]
pub struct Block {
    sender: Sender<ImageBackendRequest>,
    colors: Colors,
    handle: std::thread::JoinHandle<()>,
}

impl Backend for Block {
    fn hide(&mut self, size: Rect) -> anyhow::Result<()> {
        clear_area(&mut std::io::stdout().lock(), self.colors, size)
    }

    fn show(&mut self, data: Arc<Vec<u8>>, area: Rect) -> Result<()> {
        Ok(self.sender.send(ImageBackendRequest::Encode(EncodeRequest { area, data }))?)
    }

    fn set_config(&self, config: AlbumArtConfig) -> Result<()> {
        Ok(self.sender.send(ImageBackendRequest::SetConfig(config))?)
    }

    fn cleanup(self: Box<Self>, _area: Rect) -> Result<()> {
        self.sender.send(ImageBackendRequest::Stop)?;
        self.handle.join().expect("block thread to end gracefully");
        Ok(())
    }
}

const LOSS_FACTOR: usize = 2;
const MAX_WIDTH: u16 = 180;
const PRINT_BLOCK: &str = "\u{2580}";

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
impl Block {
    pub(super) fn new(config: AlbumArtConfig) -> Self {
        let (sender, receiver) = unbounded::<ImageBackendRequest>();
        let colors = config.colors;

        let handle = std::thread::Builder::new()
            .name("block".to_string())
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

                    let mut w = std::io::stdout().lock();
                    if !IS_SHOWING.load(Ordering::Relaxed) {
                        log::trace!(
                            "Not showing image because its not supposed to be displayed anymore"
                        );
                        continue;
                    }
                    try_cont!(
                        clear_area(&mut w, config.colors, area),
                        "Failed to clear image area"
                    );

                    let (img, resized_area) = try_cont!(
                        encode(&data, area, config.max_size, config.halign, config.valign),
                        "Failed to encode"
                    );

                    try_skip!(
                        display(&mut w, &img, resized_area),
                        "Failed to clear sixel image area"
                    );
                }
            })
            .expect("block thread to be spawned");

        Self { sender, colors, handle }
    }
}

fn encode(
    data: &[u8],
    area: Rect,
    max_size: Size,
    halign: HorizontalAlign,
    valign: VerticalAlign,
) -> Result<(DynamicImage, AlignedArea)> {
    match resize_image(data, area, max_size, halign, valign) {
        Ok(resized) => {
            if resized.1.size_px.width > MAX_WIDTH {
                status_error!(
                    "Image Size too big. Should be less than {} for compatibility reasons.",
                    MAX_WIDTH
                );
                bail!("Bailing on block image render.");
            }
            return Ok(resized);
        }
        Err(err) => bail!("Failed to resize image, err: {}", err),
    }
}

fn display(
    w: &mut impl Write,
    img: &DynamicImage,
    resized_area: AlignedArea,
) -> std::io::Result<()> {
    queue!(w, SavePosition)?;
    queue!(w, MoveTo(resized_area.area.x, resized_area.area.y))?;

    let (i_height, i_width) = img.dimensions();
    let img_buffer = img.to_rgba8();

    for y in (0..i_height).step_by(LOSS_FACTOR * 2) {
        for x in (0..i_width).step_by(LOSS_FACTOR) {
            let top = match img_buffer.get_pixel_checked(x, y) {
                Some(rgb) => rgb,
                None => &image::Rgba([0, 0, 0, 255]),
            };
            let bottom = match img_buffer.get_pixel_checked(x, y + 1) {
                Some(rgb) => rgb,
                None => &image::Rgba([0, 0, 0, 255]),
            };
            let fg = color_from_pixel(*top);
            let bg = color_from_pixel(*bottom);
            queue!(w, SetForegroundColor(fg), SetBackgroundColor(bg), Print(PRINT_BLOCK))?;
        }
        if y != i_height - 1 {
            queue!(w, ResetColor, Print("\r\n"))?;
        }
    }

    w.flush()?;
    queue!(w, RestorePosition)?;
    Ok(())
}

fn color_from_pixel(pixel: Rgba<u8>) -> Color {
    let rgb = (pixel[0], pixel[1], pixel[2]);

    if truecolor_available() {
        Color::Rgb { r: rgb.0, g: rgb.1, b: rgb.2 }
    } else {
        Color::AnsiValue(ansi256_from_rgb(rgb))
    }
}

fn truecolor_available() -> bool {
    if let Ok(value) = std::env::var("COLORTERM") {
        value.contains("truecolor") || value.contains("24bit")
    } else {
        false
    }
}
