use std::io::Write;

use anyhow::Result;
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    queue,
    style::{PrintStyledContent, Stylize},
    terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate},
};
use ratatui::layout::Rect;

use super::samples::WaveformSamples;
use crate::{
    config::{
        theme::waveform::WaveformTheme,
        waveform::{Waveform, WaveformDisplayMode},
    },
    shared::terminal::TtyWriter,
    ui::image::clear_area,
};

pub(super) fn render_waveform(
    writer: &TtyWriter,
    area: Rect,
    samples: &mut WaveformSamples,
    config: &Waveform,
    theme: &WaveformTheme,
) -> Result<()> {
    if area.width == 0 || area.height == 0 {
        return Ok(());
    }

    let mut writer = writer.lock();
    queue!(writer, BeginSynchronizedUpdate, SavePosition)?;
    clear_area(writer.by_ref(), Some(theme.background_color), area)?;

    match samples {
        WaveformSamples::Mono(samples) => {
            render_channel(writer.by_ref(), area, samples.make_contiguous(), config, theme)?;
        }

        WaveformSamples::Stereo { left, right } => {
            if matches!(config.display_mode, WaveformDisplayMode::StereoCentered) {
                render_centered_stereo(
                    writer.by_ref(),
                    area,
                    left.make_contiguous(),
                    right.make_contiguous(),
                    config,
                    theme,
                )?;
            } else {
                let (top, bottom) = stereo_areas(area);
                render_channel(writer.by_ref(), top, left.make_contiguous(), config, theme)?;
                render_channel(writer.by_ref(), bottom, right.make_contiguous(), config, theme)?;
            }
        }
    }

    queue!(writer, RestorePosition, EndSynchronizedUpdate)?;
    writer.flush()?;
    Ok(())
}

pub(super) fn stereo_areas(area: Rect) -> (Rect, Rect) {
    let top_height = area.height / 2;
    (
        Rect::new(area.x, area.y, area.width, top_height),
        Rect::new(area.x, area.y + top_height, area.width, area.height - top_height),
    )
}

pub(super) fn render_channel(
    writer: &mut impl Write,
    area: Rect,
    samples: &[f32],
    config: &Waveform,
    theme: &WaveformTheme,
) -> Result<()> {
    if area.height == 0 {
        return Ok(());
    }

    let middle = area.y + area.height / 2;
    draw_center_line(writer, area, middle, theme)?;

    // Track the min and max sample of every column.
    let envelope_by_column: Vec<_> = (0..area.width as usize)
        .map(|offset| {
            let (minimum, maximum) = column_extremes(samples, offset, area.width as usize);
            let top = sample_to_row(maximum, area, middle);
            let bottom = sample_to_row(minimum, area, middle);
            (top.min(bottom), top.max(bottom))
        })
        .collect();

    draw_envelope_columns(writer, area, &envelope_by_column, middle, config, theme)
}

fn render_centered_stereo(
    writer: &mut impl Write,
    area: Rect,
    left: &[f32],
    right: &[f32],
    config: &Waveform,
    theme: &WaveformTheme,
) -> Result<()> {
    let middle = area.y + area.height / 2;
    draw_center_line(writer, area, middle, theme)?;
    render_centered_channel(writer, area, left, middle, true, config, theme)?;
    render_centered_channel(writer, area, right, middle, false, config, theme)
}

fn draw_center_line(
    writer: &mut impl Write,
    area: Rect,
    middle: u16,
    theme: &WaveformTheme,
) -> Result<()> {
    if theme.show_center_line {
        for x in area.x..area.x + area.width {
            queue!(
                writer,
                MoveTo(x, middle),
                PrintStyledContent("─".with(theme.center_line_color).on(theme.background_color))
            )?;
        }
    }
    Ok(())
}

pub(super) fn render_centered_channel(
    writer: &mut impl Write,
    area: Rect,
    samples: &[f32],
    middle: u16,
    upwards: bool,
    config: &Waveform,
    theme: &WaveformTheme,
) -> Result<()> {
    let maximum_distance = if upwards {
        middle.saturating_sub(area.y)
    } else {
        area.bottom().saturating_sub(middle.saturating_add(1))
    };

    // Track the near and far distance of every column.
    let envelope_by_column: Vec<_> = (0..area.width as usize)
        .map(|offset| {
            let (minimum, maximum) = column_extremes(samples, offset, area.width as usize);
            let near_distance = minimum.abs().min(maximum.abs());
            let far_distance = minimum.abs().max(maximum.abs());
            let near = centered_sample_y(near_distance, maximum_distance, middle, upwards);
            let far = centered_sample_y(far_distance, maximum_distance, middle, upwards);
            (near.min(far), near.max(far))
        })
        .collect();

    draw_envelope_columns(writer, area, &envelope_by_column, middle, config, theme)
}

/// Draw a row of column envelopes.
fn draw_envelope_columns(
    writer: &mut impl Write,
    area: Rect,
    envelope_by_column: &[(u16, u16)],
    middle: u16,
    config: &Waveform,
    theme: &WaveformTheme,
) -> Result<()> {
    for (offset, &(top, bottom)) in envelope_by_column.iter().enumerate() {
        let x = area.x + offset as u16;
        let previous =
            offset.checked_sub(1).and_then(|previous| envelope_by_column.get(previous)).copied();

        let next = envelope_by_column.get(offset + 1).copied();

        if !config.render_silence && is_silent_envelope(previous, (top, bottom), next, middle) {
            continue;
        }

        draw_envelope_trace(writer, x, top, bottom, middle, config.fill_wave, theme)?;
    }
    Ok(())
}

/// Index bounds of the samples represented by one terminal column.
fn column_bounds(len: usize, column: usize, width: usize) -> (usize, usize) {
    let start = column * len / width;
    let end = ((column + 1) * len / width).max(start + 1).min(len);
    (start, end)
}

/// Minimum and maximum sample represented by one terminal column.
pub(super) fn column_extremes(samples: &[f32], column: usize, width: usize) -> (f32, f32) {
    let (start, end) = column_bounds(samples.len(), column, width);
    let bucket = &samples[start..end];
    let minimum = bucket.iter().copied().fold(f32::INFINITY, f32::min);
    let maximum = bucket.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    (minimum, maximum)
}

/// Map a sample value to the terminal row it should be plotted on.
pub(super) fn sample_to_row(sample: f32, area: Rect, middle: u16) -> u16 {
    let bottom = area.y + area.height.saturating_sub(1);
    (middle as i32 - (sample * (area.height.saturating_sub(1) as f32 / 2.0)).round() as i32)
        .clamp(area.y.into(), bottom.into()) as u16
}

pub(super) fn centered_sample_y(
    sample: f32,
    maximum_distance: u16,
    middle: u16,
    upwards: bool,
) -> u16 {
    let distance = (sample.abs().clamp(0.0, 1.0) * f32::from(maximum_distance)).round() as u16;
    if upwards { middle.saturating_sub(distance) } else { middle.saturating_add(distance) }
}

/// Treat continuous center-row runs as silence.
pub(super) fn is_silent_envelope(
    previous: Option<(u16, u16)>,
    (top, bottom): (u16, u16),
    next: Option<(u16, u16)>,
    middle: u16,
) -> bool {
    let is_center = |(top, bottom): (u16, u16)| top == middle && bottom == middle;
    top == middle
        && bottom == middle
        && previous.is_none_or(is_center)
        && next.is_none_or(is_center)
}

fn plot(writer: &mut impl Write, x: u16, y: u16, theme: &WaveformTheme) -> Result<()> {
    queue!(
        writer,
        MoveTo(x, y),
        PrintStyledContent(
            theme.trace_symbol.as_str().with(theme.trace_color).on(theme.background_color)
        )
    )?;
    Ok(())
}

/// Draw one column's envelope covering the amplitude range.
pub(super) fn draw_envelope_trace(
    writer: &mut impl Write,
    x: u16,
    top: u16,
    bottom: u16,
    middle: u16,
    fill_wave: bool,
    theme: &WaveformTheme,
) -> Result<()> {
    let (top, bottom) =
        if fill_wave { (top.min(middle), bottom.max(middle)) } else { (top, bottom) };
    for y in top..=bottom {
        plot(writer, x, y, theme)?;
    }
    Ok(())
}
