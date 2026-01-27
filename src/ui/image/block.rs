use std::io::Write;

use ansi_colours::ansi256_from_rgb;
use anyhow::Result;
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
};
use image::DynamicImage;
use ratatui::layout::Rect;

use super::Backend;
use crate::{
    config::{
        Size,
        album_art::{HorizontalAlign, VerticalAlign},
    },
    ctx::Ctx,
    shared::image::{AlignedArea, resize_image},
    ui::image::clear_area,
};

#[derive(Debug)]
pub struct Block;

#[derive(derive_more::Debug)]
pub struct Data {
    #[debug(skip)]
    image: DynamicImage,
    aligned_area: AlignedArea,
}

impl Backend for Block {
    type EncodedData = Data;

    fn hide(
        &mut self,
        w: &mut impl Write,
        size: Rect,
        bg_color: Option<crossterm::style::Color>,
    ) -> anyhow::Result<()> {
        clear_area(w, bg_color, size)
    }

    fn display(&mut self, w: &mut impl Write, data: Self::EncodedData, _ctx: &Ctx) -> Result<()> {
        queue!(w, SavePosition)?;
        let img = data.image;
        let resized_area = data.aligned_area;

        let img_buffer = img.to_rgba8();
        let area = &resized_area.area;

        let (img_width, img_height) = img_buffer.dimensions();
        let cell_width = resized_area.cell_width_px;
        let cell_height = resized_area.cell_height_px / 2.0;

        for term_y in 0..area.height {
            queue!(w, MoveTo(area.x, area.y + term_y))?;

            for term_x in 0..area.width {
                let pixel_x = term_x as f64 * cell_width;

                let top_pixel_row = term_y as f64 * 2.0 * cell_height;
                let bottom_pixel_row = top_pixel_row + cell_height;

                let top_pixel = average_region_pixel(
                    &img_buffer,
                    pixel_x as u32,
                    top_pixel_row as u32,
                    cell_width as u32,
                    cell_height as u32,
                    img_width,
                    img_height,
                );
                let bottom_pixel = average_region_pixel(
                    &img_buffer,
                    pixel_x as u32,
                    bottom_pixel_row as u32,
                    cell_width as u32,
                    cell_height as u32,
                    img_width,
                    img_height,
                );

                let top_color = color_from_pixel(top_pixel);
                let bottom_color = color_from_pixel(bottom_pixel);

                queue!(
                    w,
                    SetForegroundColor(bottom_color),
                    SetBackgroundColor(top_color),
                    Print(LOWER_HALF_BLOCK)
                )?;
            }

            queue!(w, ResetColor)?;
        }

        w.flush()?;
        queue!(w, RestorePosition)?;

        Ok(())
    }

    fn create_data(
        image_data: &[u8],
        area: Rect,
        max_size: Size,
        halign: HorizontalAlign,
        valign: VerticalAlign,
    ) -> Result<Self::EncodedData> {
        let (img, resized_area) = resize_image(image_data, area, max_size, halign, valign)?;
        Ok(Data { image: img, aligned_area: resized_area })
    }
}

const LOWER_HALF_BLOCK: &str = "\u{2584}";

#[inline]
fn average_region_pixel(
    img: &image::RgbaImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    width: u32,
    height: u32,
) -> image::Rgba<u8> {
    let x_end = x.saturating_add(w).min(width);
    let y_end = y.saturating_add(h).min(height);

    if x_end <= x || y_end <= y {
        return image::Rgba([0, 0, 0, 255]);
    }

    let mut sr: u64 = 0;
    let mut sg: u64 = 0;
    let mut sb: u64 = 0;
    let mut sa: u64 = 0;
    let mut count: u64 = 0;

    for yy in y..y_end {
        for xx in x..x_end {
            let pixel = get_pixel_safe(img, xx, yy, width, height);
            sr += pixel[0] as u64;
            sg += pixel[1] as u64;
            sb += pixel[2] as u64;
            sa += pixel[3] as u64;
            count += 1;
        }
    }

    if count == 0 {
        image::Rgba([0, 0, 0, 255])
    } else {
        image::Rgba([
            (sr / count) as u8,
            (sg / count) as u8,
            (sb / count) as u8,
            (sa / count) as u8,
        ])
    }
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
