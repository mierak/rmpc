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
    pub cell_width_px: f64,
    pub cell_height_px: f64,
}

#[derive(Debug, Clone, Copy)]
struct SizePx {
    width: f64,
    height: f64,
}

/// Returns a new aligned area contained by [`available_area`] with aspect ratio
/// provided by [`image_size`]. Constrains area by [`max_size_px`].
/// Returns the input [`available_area`] and [`max_size_px`] if terminal's size
/// cannot be determined properly. Also returns resulting area size in pixels.
pub fn create_aligned_area(
    available_area_cells: Rect,
    image_size_px: (u32, u32),
    max_size_px: Size,
    halign: HorizontalAlign,
    valign: VerticalAlign,
) -> AlignedArea {
    // Validate areas, sizes
    if image_size_px.0 == 0 || image_size_px.1 == 0 {
        log::warn!(available_area_cells:?, max_size_px:?; "Invalid image size");
        return AlignedArea {
            area: available_area_cells,
            size_px: Size { width: 0, height: 0 },
            cell_width_px: 0.0,
            cell_height_px: 0.0,
        };
    }
    if max_size_px.width == 0 || max_size_px.height == 0 {
        log::warn!(available_area_cells:?, max_size_px:?; "Max size is zero, cannot render image");
        return AlignedArea {
            area: available_area_cells,
            size_px: Size { width: 0, height: 0 },
            cell_width_px: 0.0,
            cell_height_px: 0.0,
        };
    }
    if available_area_cells.width == 0 || available_area_cells.height == 0 {
        log::warn!(available_area_cells:?, max_size_px:?; "Available area is empty, cannot render image");
        return AlignedArea {
            area: available_area_cells,
            size_px: Size { width: 0, height: 0 },
            cell_width_px: 0.0,
            cell_height_px: 0.0,
        };
    }

    // Query terminal size and calculate cell size in pixels
    let Ok(window_size) = crossterm::terminal::window_size() else {
        log::warn!(available_area_cells:?, max_size_px:?; "Failed to query terminal size");
        return AlignedArea {
            area: available_area_cells,
            size_px: max_size_px,
            cell_width_px: 0.0,
            cell_height_px: 0.0,
        };
    };

    if window_size.width == 0
        || window_size.height == 0
        || window_size.rows == 0
        || window_size.columns == 0
    {
        log::warn!(available_area_cells:?, max_size_px:?; "Terminal returned invalid size");
        return AlignedArea {
            area: available_area_cells,
            size_px: max_size_px,
            cell_width_px: 0.0,
            cell_height_px: 0.0,
        };
    }

    let cell_width = window_size.width as f64 / window_size.columns as f64;
    let cell_height = window_size.height as f64 / window_size.rows as f64;
    log::debug!(window_size:?, cell_width:?, cell_height:?; "Terminal size");

    // Convert available area to pixel space and build pixel bounds by clamping to
    // max_size_px
    let area_px = SizePx {
        width: available_area_cells.width as f64 * cell_width,
        height: available_area_cells.height as f64 * cell_height,
    };
    let bounds_px = SizePx {
        width: area_px.width.min(max_size_px.width as f64),
        height: area_px.height.min(max_size_px.height as f64),
    };

    let img_px = SizePx { width: image_size_px.0 as f64, height: image_size_px.1 as f64 };
    log::debug!(img_px:?, area_px:?, bounds_px:?, available_cells:? = available_area_cells; "Image and bounds");

    // Fit image into the pixel bounds while preserving aspect ratio
    let used_px = {
        let scale = (bounds_px.width / img_px.width).min(bounds_px.height / img_px.height);
        SizePx { width: img_px.width * scale, height: img_px.height * scale }
    };
    log::debug!(used_px:?; "Used size in pixels");

    // Convert to cell units (ceil to ensure the allocated cell box can contain the
    // used_px) Clamp to available area to guard against any floating-point edge
    // cases
    let mut used_cells = Size {
        width: (used_px.width / cell_width).ceil() as u16,
        height: (used_px.height / cell_height).ceil() as u16,
    };
    used_cells.width = used_cells.width.min(available_area_cells.width).max(1);
    used_cells.height = used_cells.height.min(available_area_cells.height).max(1);

    log::debug!(used_px:?, used_cells:?; "Used size in pixels and cells");

    let x = match halign {
        HorizontalAlign::Left => available_area_cells.x,
        HorizontalAlign::Center => {
            let offset = available_area_cells.width.saturating_sub(used_cells.width) / 2;
            available_area_cells.x + offset
        }
        HorizontalAlign::Right => {
            available_area_cells.x + available_area_cells.width.saturating_sub(used_cells.width)
        }
    };
    let y = match valign {
        VerticalAlign::Top => available_area_cells.y,
        VerticalAlign::Center => {
            let offset = available_area_cells.height.saturating_sub(used_cells.height) / 2;
            available_area_cells.y + offset
        }
        VerticalAlign::Bottom => {
            available_area_cells.y + available_area_cells.height.saturating_sub(used_cells.height)
        }
    };

    let result = AlignedArea {
        area: Rect::new(x, y, used_cells.width, used_cells.height),
        size_px: Size { width: used_px.width as u16, height: used_px.height as u16 },
        cell_width_px: cell_width,
        cell_height_px: cell_height,
    };
    log::debug!(result:?, available_area_cells:?, cell_width, cell_height, image_size_px:?, max_size_px:?, window_size:?; "Aligned area");

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
