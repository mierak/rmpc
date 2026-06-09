use anyhow::Result;
use ratatui::{Frame, layout::Rect, style::Color};

use super::Pane;
use crate::{ctx::Ctx, shared::keys::ActionEvent};

/// A procedural, per-song "album cover" pane.
///
/// Demonstrates what ratatui can really do beyond styled text: every character
/// cell is painted as an upper-half-block (`▀`) whose foreground encodes the
/// *top* sub-pixel and whose background encodes the *bottom* one. That doubles
/// the vertical resolution, and combined with truecolor per-cell output lets us
/// render a genuinely smooth, anti-band-free gradient — a real raster image
/// drawn into the terminal grid, no image protocol required.
///
/// The gradient is a duotone diagonal + soft radial glow + vignette, with hues,
/// angle and light position derived deterministically from the playing song, so
/// every track gets its own distinct, stable generative cover.
#[derive(Debug, Default)]
pub struct GradientArtPane {
    area: Rect,
}

impl GradientArtPane {
    pub fn new() -> Self {
        Self { area: Rect::default() }
    }
}

// sRGB <-> linear so colour mixing happens in linear light (correct blending).
fn srgb_to_lin(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}
fn lin_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}
fn to_lin(rgb: [u8; 3]) -> [f32; 3] {
    [srgb_to_lin(rgb[0] as f32 / 255.0), srgb_to_lin(rgb[1] as f32 / 255.0), srgb_to_lin(
        rgb[2] as f32 / 255.0,
    )]
}
fn to_color(lin: [f32; 3]) -> Color {
    let enc = |c: f32| (lin_to_srgb(c.clamp(0.0, 1.0)) * 255.0).round() as u8;
    Color::Rgb(enc(lin[0]), enc(lin[1]), enc(lin[2]))
}
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    smooth((x - edge0) / (edge1 - edge0))
}

// FNV-1a hash of the song identity -> stable per-track seed.
fn seed_of(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Curated accent palette (sRGB) the generative covers draw from.
const PALETTE: [[u8; 3]; 6] = [
    [0x28, 0xd6, 0xdf], // cyan
    [0x8b, 0x5c, 0xf6], // violet
    [0xed, 0x68, 0xae], // magenta
    [0x1f, 0xb6, 0xc9], // teal
    [0x6d, 0x6c, 0xf0], // indigo
    [0xf0, 0x6b, 0x9c], // rose
];
const BASE: [u8; 3] = [0x0c, 0x10, 0x18]; // deep ink base

impl GradientArtPane {
    fn paint(&self, frame: &mut Frame, area: Rect, seed: u64) {
        let n = PALETTE.len() as u64;
        let ai = (seed % n) as usize;
        let mut bi = ((seed >> 8) % n) as usize;
        if bi == ai {
            bi = (bi + 1) % PALETTE.len();
        }
        let lin_a = to_lin(PALETTE[ai]);
        let lin_b = to_lin(PALETTE[bi]);
        let lin_base = to_lin(BASE);
        // a near-white tint of hue A for the specular glow
        let glow_col = mix([1.0, 1.0, 1.0], lin_a, 0.45);

        let angle = ((seed >> 16) % 360) as f32 * std::f32::consts::PI / 180.0;
        let (dy, dx) = angle.sin_cos();
        let lx = 0.20 + ((seed >> 24) % 60) as f32 / 100.0; // light center x 0.20..0.80
        let ly = 0.18 + ((seed >> 32) % 55) as f32 / 100.0; // light center y 0.18..0.73

        let w = area.width as f32;
        let h2 = (area.height as f32) * 2.0; // sub-pixel rows
        let denom_x = (w - 1.0).max(1.0);
        let denom_y = (h2 - 1.0).max(1.0);

        let sample = |u: f32, v: f32| -> [f32; 3] {
            // primary diagonal duotone
            let t = smooth(0.5 + 0.5 * ((u - 0.5) * dx + (v - 0.5) * dy));
            let mut col = mix(lin_a, lin_b, t);
            // perpendicular shade toward base for depth
            let t2 = 0.5 + 0.5 * ((u - 0.5) * -dy + (v - 0.5) * dx);
            col = mix(col, lin_base, 0.18 * (1.0 - smooth(t2)));
            // soft radial specular glow
            let d = ((u - lx).powi(2) + (v - ly).powi(2)).sqrt();
            let glow = 1.0 - smoothstep(0.0, 0.72, d);
            col = [
                col[0] + glow * 0.30 * glow_col[0],
                col[1] + glow * 0.30 * glow_col[1],
                col[2] + glow * 0.30 * glow_col[2],
            ];
            // vignette toward the corners
            let ed = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt() * std::f32::consts::SQRT_2;
            let vig = 1.0 - 0.5 * smoothstep(0.45, 1.0, ed);
            [col[0] * vig, col[1] * vig, col[2] * vig]
        };

        // anti-aliased rounded corners: blend the cover into the panel ink so it
        // reads as an elevated card rather than a hard rectangle.
        let corner_bg = to_lin(BASE);
        let radius = (w.min(h2) * 0.12).clamp(2.0, 9.0);
        let (max_x, max_y) = (w - 1.0, h2 - 1.0);
        let corner_dist = |px: f32, py: f32| -> f32 {
            let dx = if px < radius {
                radius - px
            } else if px > max_x - radius {
                px - (max_x - radius)
            } else {
                0.0
            };
            let dy = if py < radius {
                radius - py
            } else if py > max_y - radius {
                py - (max_y - radius)
            } else {
                0.0
            };
            (dx * dx + dy * dy).sqrt()
        };
        let px_color = |px: u16, py: u16| -> [f32; 3] {
            let (fx, fy) = (f32::from(px), f32::from(py));
            let base = sample(fx / denom_x, fy / denom_y);
            let a = smoothstep(radius - 0.8, radius + 0.8, corner_dist(fx, fy));
            mix(base, corner_bg, a)
        };

        let buf = frame.buffer_mut();
        for ry in 0..area.height {
            for cx in 0..area.width {
                let top = px_color(cx, ry * 2);
                let bottom = px_color(cx, ry * 2 + 1);
                if let Some(cell) = buf.cell_mut((area.x + cx, area.y + ry)) {
                    cell.set_symbol("\u{2580}"); // ▀ upper half block
                    cell.set_fg(to_color(top));
                    cell.set_bg(to_color(bottom));
                }
            }
        }
    }
}

impl Pane for GradientArtPane {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &Ctx) -> Result<()> {
        self.area = area;
        if area.width == 0 || area.height == 0 {
            return Ok(());
        }
        let seed = ctx
            .current_song()
            .map_or_else(|| seed_of("rmpc-refined"), |song| seed_of(&song.file));
        self.paint(frame, area, seed);
        Ok(())
    }

    fn handle_action(&mut self, _event: &mut ActionEvent, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}
