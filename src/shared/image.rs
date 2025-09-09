use std::io::Cursor;

use anyhow::{Context, Result};
use image::{
    AnimationDecoder,
    DynamicImage,
    ImageDecoder,
    codecs::{gif::GifDecoder, jpeg::JpegEncoder},
};
use ratatui::layout::Rect;

use crate::config::{
    Size,
    album_art::{HorizontalAlign, VerticalAlign},
};

#[derive(Debug, Clone, Copy)]
pub struct AlignedArea {
    pub area: Rect,
    pub size_px: Size,
}

/// Returns a new aligned area contained by [`available_area`] with aspect ratio
/// provided by [`image_size`]. Constrains area by [`max_size_px`].
/// Returns the input [`available_area`] and [`max_size_px`] if terminal's size
/// cannot be determined properly. Also returns resulting area size in pixels.
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
            width: ((new_width as f64 * cell_width) as u16).min(max_size_px.width),
            height: ((new_height as f64 * cell_height) as u16).min(max_size_px.height),
        },
    };

    log::debug!(result:?, available_area:?, cell_width, cell_height, image_size:?, max_size_px:?, window_size:?; "Aligned area");

    result
}

pub fn resize_image(
    image_data: &[u8],
    available_area: Rect,
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
        available_area,
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

pub fn jpg_encode(img: &DynamicImage) -> Result<Vec<u8>> {
    let mut jpg = Vec::new();
    JpegEncoder::new(&mut jpg).encode_image(img)?;
    Ok(jpg)
}

pub struct GifData<'frames> {
    pub frames: image::Frames<'frames>,
    pub dimensions: (u32, u32),
}

pub fn get_gif_frames(data: &[u8]) -> Result<Option<GifData<'_>>> {
    // http://www.matthewflickinger.com/lab/whatsinagif/bits_and_bytes.asp
    if data.len() < 6 || data[0..6] != *b"GIF89a" {
        return Ok(None);
    }

    if GifDecoder::new(Cursor::new(data))?.into_frames().take(2).count() > 1 {
        let gif = GifDecoder::new(Cursor::new(data))?;
        Ok(Some(GifData { dimensions: gif.dimensions(), frames: gif.into_frames() }))
    } else {
        Ok(None)
    }
}
