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

const UPPER_HALF_BLOCK: &str = "\u{2580}";
const LOWER_HALF_BLOCK: &str = "\u{2584}";

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
