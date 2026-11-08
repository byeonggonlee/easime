//! A numbered candidate page drawn into the existing input-panel surface.
//!
//! **OS-specific: drawing.** What is listed (items, selection, page) is shared
//! (`ime_core::CandidateList`); painting it into a pixel buffer is this
//! frontend's. The painter itself needs nothing from Wayland and another frontend
//! could reuse it, but none does yet (`docs/platforms.md`).

use crate::font::Fonts;
use crate::indicator::Bitmap;
use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use ime_core::{CandidateList, Language};

/// The narrowest window, and the widest: a long candidate or annotation makes
/// the window wider up to the limit, past which the text ends in an ellipsis,
/// so what is cut off is visible as cut off.
const WIDTH: u32 = 400;
const MAX_WIDTH: u32 = 960;
/// Space left and right of the text.
const MARGIN: f32 = 12.0;
const ROW: u32 = 34;
const HEADER: u32 = 34;
const FOOTER: u32 = 30;
const BACKGROUND: [u8; 3] = [0x20, 0x22, 0x26];
const TEXT: [u8; 3] = [0xf2, 0xf2, 0xf2];
const SELECTED: [u8; 3] = [0x35, 0x55, 0x75];

pub struct Painter {
    korean: Option<FontVec>,
    japanese: Option<FontVec>,
    latin: Option<FontVec>,
}

/// The first of `fonts` that has a glyph for `ch`.
fn font_in<'a>(fonts: &[&'a FontVec], ch: char) -> Option<&'a FontVec> {
    fonts.iter().copied().find(|font| font.glyph_id(ch).0 != 0)
}

impl Painter {
    pub fn new() -> Self {
        let fonts = Fonts::load_system();
        Self {
            korean: fonts.font_for('한'),
            japanese: fonts.font_for('あ'),
            latin: fonts.font_for('A'),
        }
    }

    pub fn available(&self) -> bool {
        font_in(&self.chain(Language::Korean), 'A').is_some()
    }

    /// The fonts to try for a character, best first. The same Han character
    /// has a different shape in Japanese and in Korean, so the font of the
    /// language being converted comes first.
    fn chain(&self, language: Language) -> Vec<&FontVec> {
        let (first, second) = match language {
            Language::Japanese => (&self.japanese, &self.korean),
            _ => (&self.korean, &self.japanese),
        };
        first
            .iter()
            .chain(second.iter())
            .chain(self.latin.iter())
            .collect()
    }

    pub fn render(
        &self,
        list: &CandidateList,
        reading: &str,
        language: Language,
    ) -> Option<Bitmap> {
        let fonts = self.chain(language);
        if font_in(&fonts, 'A').is_none()
            || list.items.is_empty()
            || list.selected >= list.items.len()
        {
            return None;
        }
        let (start, end) = (list.page_start(), list.page_end());
        let height = HEADER + (end - start) as u32 * ROW + FOOTER;
        let labels: Vec<String> = (start..end)
            .map(|index| Self::label(&fonts, &list.items[index]))
            .collect();
        let footer = format!(
            "{}/{}  PgUp/PgDn  1-9/Enter  Esc",
            list.page_number(),
            list.page_count()
        );
        let width = Self::width_for(&fonts, list, reading, &labels, &footer);
        let mut bitmap = Bitmap {
            width,
            height,
            stride: width * 4,
            data: [BACKGROUND[2], BACKGROUND[1], BACKGROUND[0], 255]
                .repeat((width * height) as usize),
        };
        self.text(&fonts, &mut bitmap, reading, 12.0, 24.0, 17.0, TEXT);
        for (row, index) in (start..end).enumerate() {
            let top = HEADER + row as u32 * ROW;
            if index == list.selected {
                for y in top..top + ROW {
                    for x in 4..width - 4 {
                        let offset = ((y * width + x) * 4) as usize;
                        bitmap.data[offset..offset + 3].copy_from_slice(&[
                            SELECTED[2],
                            SELECTED[1],
                            SELECTED[0],
                        ]);
                    }
                }
            }
            let number = if index == list.selected {
                format!("> {}", row + 1)
            } else {
                format!("  {}", row + 1)
            };
            self.text(
                &fonts,
                &mut bitmap,
                &number,
                10.0,
                (top + 24) as f32,
                17.0,
                TEXT,
            );
            let label = &labels[row];
            let candidate_end = self.text(
                &fonts,
                &mut bitmap,
                label,
                52.0,
                (top + 25) as f32,
                23.0,
                TEXT,
            );
            if let Some(annotation) = list.annotations.get(index) {
                self.text(
                    &fonts,
                    &mut bitmap,
                    annotation,
                    (candidate_end + 12.0).max(100.0),
                    (top + 24) as f32,
                    17.0,
                    TEXT,
                );
            }
        }
        self.text(
            &fonts,
            &mut bitmap,
            &footer,
            12.0,
            (height - 10) as f32,
            14.0,
            TEXT,
        );
        Some(bitmap)
    }

    /// What is drawn for a candidate: itself, or its code points when a
    /// character has no glyph in any font (never an invisible or wrong glyph).
    fn label(fonts: &[&FontVec], item: &str) -> String {
        if item.chars().all(|ch| font_in(fonts, ch).is_some()) {
            item.to_owned()
        } else {
            item.chars()
                .map(|ch| format!("U+{:04X}", ch as u32))
                .collect::<Vec<_>>()
                .join(" ")
        }
    }

    /// How wide `text` is at `size`, the way [`Painter::text`] draws it.
    fn measure(fonts: &[&FontVec], text: &str, size: f32) -> f32 {
        text.chars()
            .map(|ch| {
                let ch = if font_in(fonts, ch).is_some() {
                    ch
                } else {
                    '?'
                };
                font_in(fonts, ch).map_or(0.0, |font| {
                    font.as_scaled(PxScale::from(size))
                        .h_advance(font.glyph_id(ch))
                })
            })
            .sum()
    }

    /// The window width that shows the whole page: at least [`WIDTH`], at most
    /// [`MAX_WIDTH`].
    fn width_for(
        fonts: &[&FontVec],
        list: &CandidateList,
        reading: &str,
        labels: &[String],
        footer: &str,
    ) -> u32 {
        let mut right = 12.0 + Self::measure(fonts, reading, 17.0);
        right = right.max(12.0 + Self::measure(fonts, footer, 14.0));
        for (row, label) in labels.iter().enumerate() {
            let candidate_end = 52.0 + Self::measure(fonts, label, 23.0);
            right = right.max(candidate_end);
            if let Some(annotation) = list.annotations.get(list.page_start() + row) {
                let start = (candidate_end + 12.0).max(100.0);
                right = right.max(start + Self::measure(fonts, annotation, 17.0));
            }
        }
        ((right + MARGIN).ceil() as u32).clamp(WIDTH, MAX_WIDTH)
    }

    #[allow(clippy::too_many_arguments)]
    fn text(
        &self,
        fonts: &[&FontVec],
        bitmap: &mut Bitmap,
        text: &str,
        mut x: f32,
        baseline: f32,
        size: f32,
        color: [u8; 3],
    ) -> f32 {
        let limit = bitmap.width as f32 - MARGIN;
        // Text that does not fit ends in an ellipsis instead of being cut off.
        let ellipsis_width = if font_in(fonts, '…').is_some() {
            Self::measure(fonts, "…", size)
        } else {
            0.0
        };
        let fits = x + Self::measure(fonts, text, size) <= limit;
        let mut chars = text.chars().collect::<Vec<_>>();
        if !fits && ellipsis_width > 0.0 {
            // Drop characters from the end until the ellipsis fits after them.
            while !chars.is_empty()
                && x + Self::measure(fonts, &chars.iter().collect::<String>(), size)
                    + ellipsis_width
                    > limit
            {
                chars.pop();
            }
            chars.push('…');
        }
        for ch in chars {
            let ch = if font_in(fonts, ch).is_some() {
                ch
            } else {
                '?'
            };
            let Some(font) = font_in(fonts, ch) else {
                continue;
            };
            let scaled = font.as_scaled(PxScale::from(size));
            let id = font.glyph_id(ch);
            let advance = scaled.h_advance(id);
            if x + advance > limit + ellipsis_width {
                break;
            }
            if let Some(outline) =
                font.outline_glyph(id.with_scale_and_position(size, point(x, baseline)))
            {
                let bounds = outline.px_bounds();
                outline.draw(|gx, gy, coverage| {
                    let px = bounds.min.x.floor() as i32 + gx as i32;
                    let py = bounds.min.y.floor() as i32 + gy as i32;
                    if px < 0 || py < 0 || px >= bitmap.width as i32 || py >= bitmap.height as i32 {
                        return;
                    }
                    let offset = (py as u32 * bitmap.stride + px as u32 * 4) as usize;
                    for (channel, fg) in [color[2], color[1], color[0]].into_iter().enumerate() {
                        let old = bitmap.data[offset + channel] as f32;
                        bitmap.data[offset + channel] =
                            (fg as f32 * coverage + old * (1.0 - coverage)).round() as u8;
                    }
                });
            }
            x += advance;
        }
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_changes_pixels_and_each_page_has_the_right_height() {
        let painter = Painter::new();
        assert!(painter.available(), "candidate tests require a system font");
        let mut list = CandidateList {
            items: (0..11).map(|n| format!("{n}")).collect(),
            ..CandidateList::default()
        };
        let first = painter.render(&list, "", Language::Korean).unwrap();
        list.selected = 1;
        let second = painter.render(&list, "", Language::Korean).unwrap();
        assert_ne!(first.data, second.data);
        list.selected = 9;
        let last_page = painter.render(&list, "", Language::Korean).unwrap();
        assert_eq!(first.height - last_page.height, 7 * ROW);
        assert_eq!(
            last_page.data.len(),
            (last_page.stride * last_page.height) as usize
        );
    }

    #[test]
    fn short_candidates_keep_the_usual_width() {
        let painter = Painter::new();
        let list = CandidateList {
            items: vec!["1".into(), "22".into()],
            annotations: vec!["a".into(), "b".into()],
            ..CandidateList::default()
        };
        assert_eq!(
            painter.render(&list, "r", Language::Korean).unwrap().width,
            WIDTH
        );
    }

    #[test]
    fn a_long_candidate_widens_the_window_so_all_of_it_shows() {
        let painter = Painter::new();
        let list = CandidateList {
            items: vec!["W".repeat(30)],
            annotations: vec!["W".repeat(20)],
            ..CandidateList::default()
        };
        let bitmap = painter.render(&list, "r", Language::Korean).unwrap();
        assert!(
            bitmap.width > WIDTH,
            "wider than the usual {WIDTH}: {}",
            bitmap.width
        );
        assert!(bitmap.width <= MAX_WIDTH);
        // The same list at the usual width would have cut it off: nothing is
        // drawn inside the right margin now, and the text reaches past 400.
        let row = HEADER;
        // Untouched pixels: the highlight of the selected row, or the window's own color.
        let background = |x: u32| {
            let offset = ((row + ROW / 2) * bitmap.stride + x * 4) as usize;
            let pixel = &bitmap.data[offset..offset + 3];
            pixel == [SELECTED[2], SELECTED[1], SELECTED[0]]
                || pixel == [BACKGROUND[2], BACKGROUND[1], BACKGROUND[0]]
        };
        assert!(
            (WIDTH..bitmap.width - 12).any(|x| !background(x)),
            "text is drawn beyond the usual width"
        );
        assert!(
            (bitmap.width - 12..bitmap.width).all(background),
            "the right margin is empty"
        );
    }

    #[test]
    fn a_candidate_beyond_the_limit_ends_in_an_ellipsis_inside_the_margin() {
        let painter = Painter::new();
        let fonts = painter.chain(Language::Korean);
        if font_in(&fonts, '…').is_none() {
            eprintln!("SKIPPED: no installed font has an ellipsis");
            return;
        }
        let long = CandidateList {
            items: vec!["W".repeat(300)],
            ..CandidateList::default()
        };
        let longer = CandidateList {
            items: vec!["W".repeat(400)],
            ..CandidateList::default()
        };
        let bitmap = painter.render(&long, "", Language::Korean).unwrap();
        assert_eq!(bitmap.width, MAX_WIDTH, "never wider than the limit");
        let offset = |x: u32, y: u32| (y * bitmap.stride + x * 4) as usize;
        let middle = HEADER + ROW / 2;
        assert!(
            (bitmap.width - 12..bitmap.width).all(|x| {
                let pixel = &bitmap.data[offset(x, middle)..offset(x, middle) + 3];
                pixel == [SELECTED[2], SELECTED[1], SELECTED[0]]
                    || pixel == [BACKGROUND[2], BACKGROUND[1], BACKGROUND[0]]
            }),
            "nothing is drawn in the right margin"
        );
        // How much is shown does not depend on how much was cut off.
        let other = painter.render(&longer, "", Language::Korean).unwrap();
        assert_eq!(bitmap.data, other.data, "both end in the same ellipsis");
    }

    #[test]
    fn empty_or_invalid_candidates_are_never_rendered() {
        let painter = Painter::new();
        assert!(
            painter
                .render(&CandidateList::default(), "", Language::Korean)
                .is_none()
        );
        let list = CandidateList {
            items: vec!["韓".into()],
            selected: 2,
            ..CandidateList::default()
        };
        assert!(painter.render(&list, "한", Language::Korean).is_none());
    }

    #[test]
    fn missing_fonts_refuse_invisible_candidates() {
        let painter = Painter {
            korean: None,
            japanese: None,
            latin: None,
        };
        let list = CandidateList {
            items: vec!["韓".into()],
            ..CandidateList::default()
        };
        assert!(painter.render(&list, "한", Language::Korean).is_none());
    }

    /// Whether the system has any font with kana, which Japanese candidates
    /// need. Without one these tests have nothing to check.
    fn has_kana(painter: &Painter) -> bool {
        font_in(&painter.chain(Language::Japanese), 'に').is_some()
    }

    fn japanese_list() -> CandidateList {
        CandidateList {
            items: vec![
                "日本".into(),
                "二本".into(),
                "にほん".into(),
                "ニホン".into(),
            ],
            reading: "にほん".into(),
            ..CandidateList::default()
        }
    }

    #[test]
    fn japanese_candidates_are_drawn_with_their_reading_in_the_header() {
        let painter = Painter::new();
        if !has_kana(&painter) {
            eprintln!("SKIPPED: no installed font has kana; Japanese drawing not checked");
            return;
        }
        let list = japanese_list();
        let with = painter.render(&list, "にほん", Language::Japanese).unwrap();
        let without = painter.render(&list, "", Language::Japanese).unwrap();
        assert_eq!(with.height, HEADER + 4 * ROW + FOOTER);
        assert_ne!(with.data, without.data, "the reading must be drawn");
    }

    #[test]
    fn japanese_candidates_are_drawn_when_a_font_has_only_what_japanese_needs() {
        // No Korean font at all: the Japanese font alone must be enough.
        let full = Painter::new();
        if !has_kana(&full) {
            eprintln!("SKIPPED: no installed font has kana; Japanese drawing not checked");
            return;
        }
        let painter = Painter {
            korean: None,
            japanese: full.japanese,
            latin: full.latin,
        };
        let list = japanese_list();
        assert!(
            painter
                .render(&list, "にほん", Language::Japanese)
                .is_some()
        );
    }

    #[test]
    fn the_language_picks_which_font_comes_first() {
        let painter = Painter::new();
        let first = |language| {
            painter
                .chain(language)
                .first()
                .map(|font| std::ptr::from_ref::<FontVec>(font))
        };
        // With both a Korean and a Japanese font installed the order flips;
        // with only one of them (or none) the chains are the same.
        if let (Some(korean), Some(japanese)) = (&painter.korean, &painter.japanese) {
            assert_eq!(
                first(Language::Japanese),
                Some(std::ptr::from_ref(japanese))
            );
            assert_eq!(first(Language::Korean), Some(std::ptr::from_ref(korean)));
        } else {
            eprintln!("SKIPPED: needs a Korean and a Japanese font to compare the order");
        }
    }
}
