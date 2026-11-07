//! The language indicator box: a small rounded square with one glyph in it
//! ("A", "한", "あ", "中"), drawn into an ARGB8888 pixel buffer.
//!
//! This module is pure (no Wayland, no file access): give it a glyph and,
//! optionally, a font, and it returns pixels. Finding a font is the job of
//! [`crate::font`]. Without a font (or if the font lacks the glyph) the box is
//! still drawn, just empty, because a missing font must never break typing.

use ab_glyph::{Font, PxScale, point};
use ime_core::Language;

/// Edge length of the box in logical pixels.
pub const BOX_SIZE: u32 = 48;

/// Background: dark and translucent (straight, not premultiplied, RGBA).
const BACKGROUND: [u8; 4] = [0x20, 0x22, 0x26, 0xE0];
/// Glyph colour: light.
const FOREGROUND: [u8; 3] = [0xF2, 0xF2, 0xF2];
/// Corner radius as a fraction of the box size.
const RADIUS: f32 = 0.22;
/// Glyph height (font size) as a fraction of the box size.
const GLYPH_SIZE: f32 = 0.64;

/// The glyph shown for a language.
pub fn glyph_for(language: Language) -> char {
    match language {
        Language::English => 'A',
        Language::Korean => '한',
        Language::Japanese => 'あ',
        Language::Chinese => '中',
    }
}

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

/// Coverage (0..=1) of the pixel centred at (px, py) by a rounded square of
/// edge `size`, from a signed distance estimate.
fn box_coverage(px: f32, py: f32, size: f32, radius: f32) -> f32 {
    let half = size / 2.0;
    let qx = (px - half).abs() - (half - radius);
    let qy = (py - half).abs() - (half - radius);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    let distance = outside + qx.max(qy).min(0.0) - radius;
    (0.5 - distance).clamp(0.0, 1.0)
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
        draw_glyph(&mut data, size, glyph, font);
    }
    Bitmap {
        width: size,
        height: size,
        stride,
        data,
    }
}

/// Composite the glyph, centred on its ink bounds, over the background.
fn draw_glyph<F: Font>(data: &mut [u8], size: u32, glyph: char, font: &F) {
    let id = font.glyph_id(glyph);
    // Glyph 0 is ".notdef" (the "tofu" box): better to show nothing.
    if id.0 == 0 {
        return;
    }
    let scale = PxScale::from(size as f32 * GLYPH_SIZE);
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
        for (k, fg) in [FOREGROUND[2], FOREGROUND[1], FOREGROUND[0]]
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

    fn empty_box() -> Bitmap {
        render::<FontVec>('A', None, BOX_SIZE)
    }

    #[test]
    fn glyphs_per_language() {
        assert_eq!(glyph_for(Language::English), 'A');
        assert_eq!(glyph_for(Language::Korean), '한');
        assert_eq!(glyph_for(Language::Japanese), 'あ');
        assert_eq!(glyph_for(Language::Chinese), '中');
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
