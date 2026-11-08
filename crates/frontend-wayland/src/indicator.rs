//! The language indicator box: a small rounded square with one glyph in it
//! ("A", "한", "あ", "中"), drawn into an ARGB8888 pixel buffer.
//!
//! This module is pure (no Wayland, no file access): give it a glyph and,
//! optionally, a font, and it returns pixels. Finding a font is the job of
//! [`crate::font`]. Without a font (or if the font lacks the glyph) the box is
//! still drawn, just empty, because a missing font must never break typing.
//!
//! **Portable drawing, shared letter.** Which glyph stands for a language is shared
//! (`ime_session::indicator`); this module only draws it.

use ab_glyph::{Font, PxScale, point};

/// Edge length of the box in logical pixels.
pub const BOX_SIZE: u32 = 48;

/// Background: dark and translucent (straight, not premultiplied, RGBA).
const BACKGROUND: [u8; 4] = [0x20, 0x22, 0x26, 0xE0];
/// Background for the task bar icon: dark and nearly opaque for crisp contrast.
const TRAY_BACKGROUND: [u8; 4] = [0x20, 0x24, 0x2C, 0xF8];
/// Subtle border on the task bar icon so the badge stands out on dark panels.
const TRAY_BORDER: [u8; 4] = [0x60, 0x68, 0x78, 0x90];
/// Glyph colour: light.
const FOREGROUND: [u8; 3] = [0xF2, 0xF2, 0xF2];
/// Glyph colour in the task bar: pure white for maximum legibility at 16–24 px.
const TRAY_FOREGROUND: [u8; 3] = [0xFF, 0xFF, 0xFF];
/// Corner radius as a fraction of the box size.
const RADIUS: f32 = 0.22;
/// Corner radius for the task bar badge.
const TRAY_RADIUS: f32 = 0.20;
/// Glyph height (font size) as a fraction of the box size.
const GLYPH_SIZE: f32 = 0.64;
/// Target ink size as a fraction of the task bar icon size (leaves ~12% margin
/// on each side so the letter fills ~76% of the icon's actual pixel height/width,
/// matching the visual weight of neighbouring 16–24 px system tray icons).
const TRAY_INK_FRACTION: f32 = 0.76;

/// The glyph shown for a language (shared by every frontend).
pub use ime_session::indicator::language_glyph as glyph_for;

/// An ARGB8888 image as `wl_shm` wants it: little-endian 32-bit words with
/// premultiplied alpha, so the bytes are B, G, R, A.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    /// Bytes per row.
    pub stride: u32,
    pub data: Vec<u8>,
}

impl Bitmap {
    /// The pixel at (x, y) as `[a, r, g, b]` (premultiplied).
    pub fn argb(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y * self.stride + x * 4) as usize;
        [
            self.data[i + 3],
            self.data[i + 2],
            self.data[i + 1],
            self.data[i],
        ]
    }

    /// True if any pixel is not fully transparent.
    pub fn has_visible_pixels(&self) -> bool {
        self.data.as_chunks::<4>().0.iter().any(|p| p[3] != 0)
    }

    /// Number of pixels brighter than the box background, i.e. belonging to
    /// the glyph (or its anti-aliased edge).
    pub fn glyph_pixels(&self) -> usize {
        let bg_light = BACKGROUND[0].max(BACKGROUND[1]).max(BACKGROUND[2]) as u32 + 16;
        self.data
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] != 0 && p[0].max(p[1]).max(p[2]) as u32 > bg_light)
            .count()
    }

    /// Un-premultiplied RGBA bytes, for writing PNG files.
    pub fn to_rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity((self.width * self.height * 4) as usize);
        for y in 0..self.height {
            for x in 0..self.width {
                let [a, r, g, b] = self.argb(x, y);
                let un = |c: u8| {
                    if a == 0 {
                        0
                    } else {
                        ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8
                    }
                };
                out.extend_from_slice(&[un(r), un(g), un(b), a]);
            }
        }
        out
    }
}

/// Signed distance from (px, py) to a rounded square of edge `size` and corner
/// `radius` (negative inside, positive outside).
fn box_sdf(px: f32, py: f32, size: f32, radius: f32) -> f32 {
    let half = size / 2.0;
    let qx = (px - half).abs() - (half - radius);
    let qy = (py - half).abs() - (half - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - radius
}

/// Coverage (0..=1) of the pixel centred at (px, py) by a rounded square of
/// edge `size`, from a signed distance estimate.
fn box_coverage(px: f32, py: f32, size: f32, radius: f32) -> f32 {
    (0.5 - box_sdf(px, py, size, radius)).clamp(0.0, 1.0)
}

/// Draw the box with `glyph` in it. `font` may be `None`, or lack the glyph;
/// the box is then drawn without a glyph.
pub fn render<F: Font>(glyph: char, font: Option<&F>, size: u32) -> Bitmap {
    let size = size.max(8);
    let stride = size * 4;
    let mut data = vec![0u8; (stride * size) as usize];

    // Background with anti-aliased rounded corners, premultiplied.
    let radius = size as f32 * RADIUS;
    for y in 0..size {
        for x in 0..size {
            let c = box_coverage(x as f32 + 0.5, y as f32 + 0.5, size as f32, radius);
            let a = BACKGROUND[3] as f32 / 255.0 * c;
            let i = (y * stride + x * 4) as usize;
            data[i] = (BACKGROUND[2] as f32 * a).round() as u8;
            data[i + 1] = (BACKGROUND[1] as f32 * a).round() as u8;
            data[i + 2] = (BACKGROUND[0] as f32 * a).round() as u8;
            data[i + 3] = (a * 255.0).round() as u8;
        }
    }

    if let Some(font) = font {
        let scale = PxScale::from(size as f32 * GLYPH_SIZE);
        draw_glyph(&mut data, size, glyph, font, scale, FOREGROUND);
    }
    Bitmap {
        width: size,
        height: size,
        stride,
        data,
    }
}

/// Draw the task bar badge with `glyph` in it.
///
/// Unlike the 48×48 cursor-adjacent popup (`render`), the task bar icon is
/// shown at 16–24 px alongside full-bleed system tray icons. It therefore
/// scales the glyph so its actual ink bounding box fills [`TRAY_INK_FRACTION`]
/// of the badge, uses pure white ink, and adds a subtle 1 px border so the
/// badge remains distinct on dark panels.
pub fn render_tray<F: Font>(glyph: char, font: Option<&F>, size: u32) -> Bitmap {
    let size = size.max(8);
    let stride = size * 4;
    let mut data = vec![0u8; (stride * size) as usize];

    let radius = size as f32 * TRAY_RADIUS;
    for y in 0..size {
        for x in 0..size {
            let sdf = box_sdf(x as f32 + 0.5, y as f32 + 0.5, size as f32, radius);
            let outer = (0.5 - sdf).clamp(0.0, 1.0);
            let inner = (0.5 - (sdf + 1.0)).clamp(0.0, 1.0);
            let border = (outer - inner).clamp(0.0, 1.0);

            let bg_a = TRAY_BACKGROUND[3] as f32 / 255.0 * inner;
            let bd_a = TRAY_BORDER[3] as f32 / 255.0 * border;
            let a = bg_a + bd_a;
            let r = TRAY_BACKGROUND[0] as f32 * bg_a + TRAY_BORDER[0] as f32 * bd_a;
            let g = TRAY_BACKGROUND[1] as f32 * bg_a + TRAY_BORDER[1] as f32 * bd_a;
            let b = TRAY_BACKGROUND[2] as f32 * bg_a + TRAY_BORDER[2] as f32 * bd_a;

            let i = (y * stride + x * 4) as usize;
            data[i] = b.round().clamp(0.0, 255.0) as u8;
            data[i + 1] = g.round().clamp(0.0, 255.0) as u8;
            data[i + 2] = r.round().clamp(0.0, 255.0) as u8;
            data[i + 3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }

    if let Some(font) = font {
        let scale = tray_glyph_scale(glyph, font, size);
        draw_glyph(&mut data, size, glyph, font, scale, TRAY_FOREGROUND);
    }
    Bitmap {
        width: size,
        height: size,
        stride,
        data,
    }
}

/// Choose a `PxScale` so the glyph's actual ink bounding box fills
/// [`TRAY_INK_FRACTION`] of `size` (clamped so neither width nor height
/// overflows the badge).
fn tray_glyph_scale<F: Font>(glyph: char, font: &F, size: u32) -> PxScale {
    let id = font.glyph_id(glyph);
    let ref_px = size as f32;
    let probe = id.with_scale_and_position(PxScale::from(ref_px), point(0.0, 0.0));
    if let Some(outlined) = font.outline_glyph(probe) {
        let bounds = outlined.px_bounds();
        let max_dim = bounds.width().max(bounds.height());
        if max_dim.is_finite() && max_dim > 0.0 {
            let target = size as f32 * TRAY_INK_FRACTION;
            let scaled = (ref_px * target / max_dim).clamp(ref_px * 0.64, ref_px * 1.35);
            return PxScale::from(scaled);
        }
    }
    PxScale::from(size as f32 * 0.85)
}

/// Composite the glyph, centred on its ink bounds, over the background.
fn draw_glyph<F: Font>(
    data: &mut [u8],
    size: u32,
    glyph: char,
    font: &F,
    scale: PxScale,
    foreground: [u8; 3],
) {
    let id = font.glyph_id(glyph);
    // Glyph 0 is ".notdef" (the "tofu" box): better to show nothing.
    if id.0 == 0 {
        return;
    }
    // First pass at the origin only to measure the ink.
    let probe = id.with_scale_and_position(scale, point(0.0, 0.0));
    let Some(outlined) = font.outline_glyph(probe) else {
        return;
    };
    let bounds = outlined.px_bounds();
    let (w, h) = (bounds.width(), bounds.height());
    if !(w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return;
    }
    let origin = point(
        (size as f32 - w) / 2.0 - bounds.min.x,
        (size as f32 - h) / 2.0 - bounds.min.y,
    );
    // Draw again at the final origin, which keeps the sub-pixel offset.
    let placed = id.with_scale_and_position(scale, origin);
    let Some(outlined) = font.outline_glyph(placed) else {
        return;
    };
    let bounds = outlined.px_bounds();
    outlined.draw(|gx, gy, coverage| {
        let x = bounds.min.x.floor() as i64 + gx as i64;
        let y = bounds.min.y.floor() as i64 + gy as i64;
        if x < 0 || y < 0 || x >= size as i64 || y >= size as i64 {
            return;
        }
        let i = (y as u32 * size * 4 + x as u32 * 4) as usize;
        let c = coverage.clamp(0.0, 1.0);
        // Source-over with a premultiplied destination; the glyph is opaque.
        let a = data[i + 3] as f32 / 255.0;
        let out_a = c + a * (1.0 - c);
        for (k, fg) in [foreground[2], foreground[1], foreground[0]]
            .into_iter()
            .enumerate()
        {
            let v = fg as f32 * c + data[i + k] as f32 * (1.0 - c);
            data[i + k] = v.round().clamp(0.0, 255.0) as u8;
        }
        data[i + 3] = (out_a * 255.0).round() as u8;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use ab_glyph::FontVec;
    use ime_core::Language;

    fn empty_box() -> Bitmap {
        render::<FontVec>('A', None, BOX_SIZE)
    }

    #[test]
    fn glyphs_per_language() {
        // The mapping itself is tested in ime-session; the box must be able to
        // draw each of its glyphs.
        for language in [Language::English, Language::Korean, Language::Japanese] {
            assert_ne!(glyph_for(language), ' ');
        }
    }

    #[test]
    fn size_and_stride_match_wl_shm_argb8888() {
        let b = empty_box();
        assert_eq!((b.width, b.height), (BOX_SIZE, BOX_SIZE));
        assert_eq!(b.stride, BOX_SIZE * 4);
        assert_eq!(b.data.len(), (BOX_SIZE * BOX_SIZE * 4) as usize);
    }

    #[test]
    fn box_without_a_font_is_a_visible_empty_square() {
        let b = empty_box();
        assert!(b.has_visible_pixels());
        assert_eq!(b.glyph_pixels(), 0);
    }

    #[test]
    fn corners_are_transparent_and_the_inside_is_translucent() {
        let b = empty_box();
        assert_eq!(b.argb(0, 0)[0], 0, "rounded corner");
        assert_eq!(b.argb(BOX_SIZE - 1, BOX_SIZE - 1)[0], 0, "rounded corner");
        let [a, r, g, bl] = b.argb(BOX_SIZE / 2, BOX_SIZE / 2);
        assert_eq!(a, BACKGROUND[3]);
        assert!(a < 255, "background must be translucent");
        // Premultiplied: colour never exceeds alpha.
        assert!(r <= a && g <= a && bl <= a);
        // Dark: well below the glyph colour.
        assert!(r < 0x40 && g < 0x40 && bl < 0x40);
    }

    #[test]
    fn every_pixel_is_valid_premultiplied() {
        let b = empty_box();
        for p in b.data.as_chunks::<4>().0.iter() {
            assert!(p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]);
        }
    }

    #[test]
    fn edge_midpoints_are_opaque_background_not_clipped() {
        let b = empty_box();
        for (x, y) in [
            (BOX_SIZE / 2, 0),
            (BOX_SIZE / 2, BOX_SIZE - 1),
            (0, BOX_SIZE / 2),
            (BOX_SIZE - 1, BOX_SIZE / 2),
        ] {
            assert_eq!(b.argb(x, y)[0], BACKGROUND[3], "edge pixel {x},{y}");
        }
    }

    #[test]
    fn other_sizes_scale_the_buffer() {
        let b = render::<FontVec>('A', None, 96);
        assert_eq!((b.width, b.stride), (96, 384));
        assert_eq!(b.data.len(), 96 * 384);
    }

    #[test]
    fn rgba_conversion_undoes_premultiplication() {
        let b = empty_box();
        let rgba = b.to_rgba();
        assert_eq!(rgba.len(), b.data.len());
        let i = ((BOX_SIZE / 2 * b.stride) + BOX_SIZE / 2 * 4) as usize;
        let px = &rgba[i..i + 4];
        assert_eq!(px[3], BACKGROUND[3]);
        for k in 0..3 {
            assert!((px[k] as i32 - BACKGROUND[k] as i32).abs() <= 2);
        }
        // Fully transparent pixels stay all zero.
        assert_eq!(&rgba[0..4], &[0, 0, 0, 0]);
    }

    /// A system font with `ch`, or `None` (then the test skips itself).
    fn any_system_font(ch: char) -> Option<FontVec> {
        let found = crate::font::find_font(ch);
        if found.is_none() {
            eprintln!("SKIPPED: no installed font contains {ch:?}; glyph pixels not checked");
        }
        found
    }

    #[test]
    fn latin_a_is_drawn_and_centred_when_any_font_exists() {
        let Some(font) = any_system_font('A') else {
            return;
        };
        let b = render('A', Some(&font), BOX_SIZE);
        assert!(
            b.glyph_pixels() > 20,
            "got {} glyph pixels",
            b.glyph_pixels()
        );
        assert_ink_centred(&b);
    }

    #[test]
    fn cjk_glyphs_are_drawn_when_a_cjk_font_exists() {
        for ch in ['한', 'あ', '中'] {
            let Some(font) = any_system_font(ch) else {
                continue;
            };
            let b = render(ch, Some(&font), BOX_SIZE);
            assert!(
                b.glyph_pixels() > 40,
                "{ch}: {} glyph pixels",
                b.glyph_pixels()
            );
            assert_ink_centred(&b);
        }
    }

    #[test]
    fn a_glyph_the_font_lacks_draws_no_tofu() {
        let Some(font) = any_system_font('A') else {
            return;
        };
        // A plane-16 private-use codepoint that no real font maps.
        let b = render('\u{10FFFD}', Some(&font), BOX_SIZE);
        assert_eq!(b.glyph_pixels(), 0);
    }

    #[test]
    fn tray_badge_draws_a_larger_letter_than_the_cursor_box_at_taskbar_sizes() {
        let Some(font) = any_system_font('A') else {
            return;
        };
        for size in [16, 22, 24, 32] {
            let popup = render('A', Some(&font), size);
            let tray = render_tray('A', Some(&font), size);
            assert!(
                tray.glyph_pixels() > popup.glyph_pixels(),
                "size {size}: tray={} popup={}",
                tray.glyph_pixels(),
                popup.glyph_pixels()
            );
            assert_ink_centred(&tray);
        }
    }

    /// The bright pixels' centre of mass is near the middle of the box.
    fn assert_ink_centred(b: &Bitmap) {
        let (mut sx, mut sy, mut n) = (0.0f32, 0.0f32, 0.0f32);
        for y in 0..b.height {
            for x in 0..b.width {
                let p = b.argb(x, y);
                if p[0] != 0 && p[1].max(p[2]).max(p[3]) > 0x70 {
                    sx += x as f32;
                    sy += y as f32;
                    n += 1.0;
                }
            }
        }
        assert!(n > 0.0, "no ink");
        let (cx, cy) = (sx / n, sy / n);
        let mid = (b.width as f32 - 1.0) / 2.0;
        assert!(
            (cx - mid).abs() < 4.0 && (cy - mid).abs() < 4.0,
            "ink centre {cx},{cy}"
        );
    }
}
