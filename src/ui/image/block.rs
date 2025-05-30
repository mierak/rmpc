use super::{AlbumArtConfig, Backend, ImageBackendRequest};
use crate::{
    shared::{
        image::{AlignedArea, resize_image},
        macros::try_cont,
    },
    try_skip,
    ui::image::{EncodeRequest, clear_area, facade::IS_SHOWING, recv_data},
};
use ansi_colours::ansi256_from_rgb;
use anyhow::Result;
use crossbeam::channel::{Sender, unbounded};
use crossterm::style::{ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    queue,
    style::{Color, Colors, Print},
};
use image::DynamicImage;
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

                    let mut w = std::io::stdout().lock();

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
                        resize_image(&data, area, config.max_size, config.halign, config.valign),
                        "Failed to resize block image"
                    );

                    try_skip!(display(&mut w, &img, resized_area), "Failed to display block");
                }
            })
            .expect("block thread to be spawned");

        Self { sender, colors, handle }
    }
}

const UPPER_HALF_BLOCK: &str = "\u{2580}";
const LOWER_HALF_BLOCK: &str = "\u{2584}";

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn display(
    w: &mut impl Write,
    img: &DynamicImage,
    resized_area: AlignedArea,
) -> std::io::Result<()> {
    queue!(w, SavePosition)?;

    let img_buffer = img.to_rgba8();
    let area = &resized_area.area;
    let size_px = &resized_area.size_px;

    let cell_width = (f64::from(size_px.width) / f64::from(area.width)) as u32;
    let cell_height = (f64::from(size_px.height) / (f64::from(area.height) * 2.0)) as u32;

    let (img_width, img_height) = img_buffer.dimensions();

    for term_y in 0..area.height {
        queue!(w, MoveTo(area.x, area.y + term_y))?;

        let top_pixel_row = u32::from(term_y) * 2 * cell_height;
        let bottom_pixel_row = top_pixel_row + cell_height;
        let is_last_term_row = term_y == area.height - 1;

        for term_x in 0..area.width {
            let pixel_x = u32::from(term_x) * cell_width;

            let top_pixel =
                get_pixel_safe(&img_buffer, pixel_x, top_pixel_row, img_width, img_height);
            let bottom_pixel =
                get_pixel_safe(&img_buffer, pixel_x, bottom_pixel_row, img_width, img_height);

            let top_color = color_from_pixel(top_pixel);
            let bottom_color = color_from_pixel(bottom_pixel);

            if is_last_term_row {
                // Last row - only show top pixel, bottom is empty
                queue!(w, SetForegroundColor(top_color), Print(UPPER_HALF_BLOCK))?;
            } else {
                // Normal rows - both pixels, use lower half block with fg/bg
                queue!(
                    w,
                    SetForegroundColor(bottom_color),
                    SetBackgroundColor(top_color),
                    Print(LOWER_HALF_BLOCK)
                )?;
            }
        }

        queue!(w, ResetColor)?;
    }

    w.flush()?;
    queue!(w, RestorePosition)?;

    Ok(())
}

#[inline]
fn get_pixel_safe(
    img: &image::RgbaImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> image::Rgba<u8> {
    if x < width && y < height { *img.get_pixel(x, y) } else { image::Rgba([0, 0, 0, 255]) }
}

#[inline]
fn truecolor_available() -> bool {
    if let Ok(value) = std::env::var("COLORTERM") {
        value.contains("truecolor") || value.contains("24bit")
    } else {
        false
    }
}

#[inline]
fn color_from_pixel(pixel: image::Rgba<u8>) -> Color {
    if truecolor_available() {
        Color::Rgb { r: pixel[0], g: pixel[1], b: pixel[2] }
    } else {
        Color::AnsiValue(ansi256_from_rgb((pixel[0], pixel[1], pixel[2])))
    }
}
