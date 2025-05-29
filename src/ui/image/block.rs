use super::{AlbumArtConfig, Backend, ImageBackendRequest, clear_area};
use crate::{
    config::{
        Size,
        album_art::{HorizontalAlign, VerticalAlign},
    },
    shared::{image::AlignedArea, macros::try_cont},
    try_skip,
    ui::image::{EncodeRequest, facade::IS_SHOWING, recv_data},
};
use ansi_colours::ansi256_from_rgb;
use anyhow::{Context, Result, bail};
use crossbeam::channel::{Sender, unbounded};
use crossterm::style::{SetBackgroundColor, SetForegroundColor};
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    queue,
    style::{Color, Colors, Print},
};
use image::{DynamicImage, GenericImageView, Rgba};
use ratatui::layout::Rect;
use std::io::{Cursor, Write};
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

#[allow(clippy::cast_lossless, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn create_aligned_area(
    available_area: Rect,
    image_size: (u32, u32),
    max_size_px: Size,
    halign: HorizontalAlign,
    valign: VerticalAlign,
) -> AlignedArea {
    let Ok(window_size) = crossterm::terminal::window_size() else {
        log::warn!(available_area:?, max_size_px:?; "Failed to query terminal size");
        return AlignedArea { area: available_area, size_px: max_size_px };
    };

    if window_size.width == 0 || window_size.height == 0 {
        log::warn!(available_area:?, max_size_px:?; "Terminal returned invalid size");
        return AlignedArea { area: available_area, size_px: max_size_px };
    }

    let available_width = available_area.width as f64;
    let available_height = available_area.height as f64;
    let cell_width = window_size.width as f64 / window_size.columns as f64;
    let cell_height = window_size.height as f64 / window_size.rows as f64;

    let image_aspect_ratio = image_size.0 as f64 / image_size.1 as f64;
    let cell_aspect_ratio = cell_width / cell_height;
    let available_area_aspect_ratio = available_width / available_height * cell_aspect_ratio;

    let (mut new_width, mut new_height) = if available_area_aspect_ratio < image_aspect_ratio {
        let new_width = available_area.width;
        let new_height = (available_width / image_aspect_ratio * cell_aspect_ratio).ceil() as u16;

        (new_width, new_height)
    } else {
        let new_width = (available_height * image_aspect_ratio / cell_aspect_ratio).ceil() as u16;
        let new_height = available_area.height;

        (new_width, new_height)
    };

    if new_width > available_area.width {
        new_width = available_area.width;
    }
    if new_height > available_area.height {
        new_height = available_area.height;
    }

    let new_x = match halign {
        HorizontalAlign::Left => available_area.x,
        HorizontalAlign::Center => {
            available_area.x + (available_area.width.saturating_sub(new_width)) / 2
        }
        HorizontalAlign::Right => available_area.right().saturating_sub(new_width),
    };
    let new_y = match valign {
        VerticalAlign::Top => available_area.y,
        VerticalAlign::Center => {
            available_area.y + (available_area.height.saturating_sub(new_height)) / 2
        }
        VerticalAlign::Bottom => available_area.bottom().saturating_sub(new_height),
    };

    let result = AlignedArea {
        area: Rect::new(new_x, new_y, new_width, new_height),
        size_px: Size {
            width: (new_width * 2).min(max_size_px.width),
            height: (new_height * 2).min(max_size_px.height),
        },
    };

    log::debug!(result:?, available_area:?, cell_width, cell_height, image_size:?, max_size_px:?, window_size:?; "|||||||||||||||||||||||||||||||Aligned area");

    result
}

pub fn resize_image(
    image_data: &[u8],
    availabe_area: Rect,
    max_size_px: Size,
    halign: HorizontalAlign,
    valign: VerticalAlign,
) -> Result<(DynamicImage, AlignedArea)> {
    let image = image::ImageReader::new(Cursor::new(image_data))
        .with_guessed_format()
        .context("Unable to guess image format")?
        .decode()
        .context("Unable to decode image")?;

    let result_area = create_aligned_area(
        availabe_area,
        (image.width(), image.height()),
        max_size_px,
        halign,
        valign,
    );

    let result = image.resize(
        result_area.size_px.width.into(),
        result_area.size_px.height.into(),
        image::imageops::FilterType::Lanczos3,
    );

    Ok((result, result_area))
}

fn encode(
    data: &[u8],
    area: Rect,
    max_size: Size,
    halign: HorizontalAlign,
    valign: VerticalAlign,
) -> Result<(DynamicImage, AlignedArea)> {
    match resize_image(data, area, max_size, halign, valign) {
        Ok(resized) => Ok(resized),
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

    let mut y_step = 1;
    for y in (0..i_height).step_by(2) {
        for x in 0..i_width {
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
        queue!(w, MoveTo(resized_area.area.x, resized_area.area.y + y_step))?;
        y_step += 1;
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
