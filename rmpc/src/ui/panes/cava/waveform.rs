use std::{io::Write, ops::Range};

use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    queue,
    style::{PrintStyledContent, Stylize},
};
use ratatui::layout::Rect;

use crate::config::theme::cava::CavaTheme;

/// Draw CAVA's interleaved base/top ranges, measured in rows from the bottom.
/// The trace uses half-cell pixels, with lower and upper glyphs for the
/// partial edges.
pub(super) fn render(
    writer: &mut impl Write,
    area: Rect,
    columns: &[f32],
    x_offset: u16,
    empty_bar_symbol: &str,
    theme: &CavaTheme,
) -> Result<()> {
    if area.width == 0 || area.height == 0 {
        return Ok(());
    }

    for (column, values) in columns.as_chunks::<2>().0.iter().enumerate() {
        let x = area.x + x_offset + column as u16 * (theme.bar_width + theme.bar_spacing);
        let pixels = snap(values[0], values[1], area.height);

        // Redraw the whole column so moving ranges leave no stale trace behind.
        for row in 0..area.height {
            let bottom = u32::from(row) * 2;
            let lower = pixels.contains(&bottom);
            let upper = pixels.contains(&(bottom + 1));
            let (symbols, fill, reverse) = match (lower, upper) {
                (true, true) => (&theme.bar_symbols, 1.0, false),
                (true, false) => (&theme.bar_symbols, 0.5, false),
                (false, true) => (&theme.inverted_bar_symbols, 0.5, theme.inverted_bar_as_mask),
                (false, false) => (&theme.bar_symbols, 0.0, false),
            };

            queue!(writer, MoveTo(x, area.y + area.height - 1 - row))?;
            if fill == 0.0 || symbols.is_empty() {
                queue!(writer, PrintStyledContent(empty_bar_symbol.on(theme.bg_color)))?;
            } else {
                let steps = (fill * symbols.len() as f32).round() as usize;
                let glyph = &symbols[steps.clamp(1, symbols.len()) - 1];
                let color = theme.bar_color.get_color(row as usize, area.height);
                let content = glyph.as_str().with(color).on(theme.bg_color);
                let content = if reverse { content.reverse() } else { content };
                queue!(writer, PrintStyledContent(content))?;
            }
        }
    }

    Ok(())
}

/// Snap relative to the center so mirrored spans stay mirrored, with border
/// ties going away from zero. A thin range occupies the pixel containing its
/// midpoint, including CAVA's one-raw-unit silence trace.
fn snap(low: f32, high: f32, height: u16) -> Range<u32> {
    let total = f32::from(height) * 2.0;
    let zero = f32::from(height);
    let (low, high) = (low * 2.0, high * 2.0);

    if high - low < 1.0 {
        let distance = f32::midpoint(low, high) - zero;
        let pixel =
            zero + if distance > 0.0 { distance.floor() } else { -1.0 - (-distance).floor() };
        let bottom = pixel.clamp(0.0, (total - 1.0).max(0.0));
        bottom as u32..(bottom + 1.0).min(total) as u32
    } else {
        let bottom = (zero + (low - zero).round()).clamp(0.0, total);
        let top = (zero + (high - zero).round()).clamp(0.0, total);
        bottom as u32..top as u32
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use ratatui::layout::Rect;

    use super::{render, snap};
    use crate::config::theme::cava::CavaTheme;

    fn draw(area: Rect, columns: &[f32], theme: &CavaTheme) -> String {
        let mut writer = Vec::new();
        render(&mut writer, area, columns, 0, &" ".repeat(theme.bar_width as usize), theme)
            .unwrap();

        String::from_utf8(writer).unwrap()
    }

    fn contains_at(output: &str, position: &str, glyph: char) -> bool {
        output
            .split(position)
            .nth(1)
            .and_then(|rest| rest.split('H').next())
            .is_some_and(|cell| cell.contains(glyph))
    }

    #[test]
    fn spans_snap_symmetrically_and_clip_to_the_area() {
        assert_eq!(snap(4.0, 4.0, 8), 7..8);
        assert_eq!(snap(6.0, 6.0, 8), 12..13);
        assert_eq!(snap(2.0, 2.0, 8), 3..4);
        assert_eq!(snap(5.75, 6.25, 8), 12..13);
        assert_eq!(snap(1.75, 2.25, 8), 3..4);
        assert_eq!(snap(-1.0, -1.0, 8), 0..1);
        assert_eq!(snap(9.0, 9.0, 8), 15..16);
        assert_eq!(snap(0.0, 8.0, 8), 0..16);
        assert_eq!(snap(0.0, 0.0, 0), 0..0);
    }

    #[test]
    fn draws_partial_edges_without_filling_to_zero() {
        let output = draw(Rect::new(5, 4, 1, 8), &[5.5, 7.5], &CavaTheme::default());
        assert_eq!(output.matches('▀').count(), 1);
        assert_eq!(output.matches('▄').count(), 1);
        assert_eq!(output.matches('█').count(), 1);
        assert_eq!(output.matches(' ').count(), 5);

        // Top edge is the lower half of row zero. Bottom edge is the upper
        // half of row two. Coordinates include the pane's nonzero origin.
        assert!(contains_at(&output, "\u{1b}[5;6H", '▄'));
        assert!(contains_at(&output, "\u{1b}[7;6H", '▀'));
    }

    #[test]
    fn raw_silence_preserves_the_center_trace_after_decoding() {
        // CAVA's 16-bit range has an even usable height of 65534 units.
        // Silence covers the raw pixel immediately below its center border.
        for height in [1, 8, 9] {
            let scale = f32::from(height) / 65535.0;
            assert_eq!(
                snap(32766.0 * scale, 32767.0 * scale, height),
                u32::from(height) - 1..u32::from(height)
            );
        }
    }
}
