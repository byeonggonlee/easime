//! A numbered candidate page drawn into the existing input-panel surface.

use crate::font::Fonts;
use crate::indicator::Bitmap;
use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use ime_core::CandidateList;

const WIDTH: u32 = 400;
const ROW: u32 = 34;
const HEADER: u32 = 34;
const FOOTER: u32 = 30;
const BACKGROUND: [u8; 3] = [0x20, 0x22, 0x26];
const TEXT: [u8; 3] = [0xf2, 0xf2, 0xf2];
const SELECTED: [u8; 3] = [0x35, 0x55, 0x75];

pub struct Painter {
    korean: Option<FontVec>,
    latin: Option<FontVec>,
}

impl Painter {
    pub fn new() -> Self {
        let fonts = Fonts::load_system();
        Self {
            korean: fonts.font_for('한'),
            latin: fonts.font_for('A'),
        }
    }

    pub fn available(&self) -> bool {
        self.font('A').is_some()
    }

    fn font(&self, ch: char) -> Option<&FontVec> {
        self.korean
            .iter()
            .chain(self.latin.iter())
            .find(|font| font.glyph_id(ch).0 != 0)
    }

    pub fn render(&self, list: &CandidateList, reading: &str) -> Option<Bitmap> {
        if !self.available() || list.items.is_empty() || list.selected >= list.items.len() {
            return None;
        }
        let start = list.selected / CandidateList::PAGE_SIZE * CandidateList::PAGE_SIZE;
        let end = (start + CandidateList::PAGE_SIZE).min(list.items.len());
        let height = HEADER + (end - start) as u32 * ROW + FOOTER;
        let mut bitmap = Bitmap {
            width: WIDTH,
            height,
            stride: WIDTH * 4,
            data: [BACKGROUND[2], BACKGROUND[1], BACKGROUND[0], 255]
                .repeat((WIDTH * height) as usize),
        };
        self.text(&mut bitmap, reading, 12.0, 24.0, 17.0, TEXT);
        for (row, index) in (start..end).enumerate() {
            let top = HEADER + row as u32 * ROW;
            if index == list.selected {
                for y in top..top + ROW {
                    for x in 4..WIDTH - 4 {
                        let offset = ((y * WIDTH + x) * 4) as usize;
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
            self.text(&mut bitmap, &number, 10.0, (top + 24) as f32, 17.0, TEXT);
            let item = &list.items[index];
            let label = if item.chars().all(|ch| self.font(ch).is_some()) {
                item.clone()
            } else {
                item.chars()
                    .map(|ch| format!("U+{:04X}", ch as u32))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let candidate_end = self.text(&mut bitmap, &label, 52.0, (top + 25) as f32, 23.0, TEXT);
            if let Some(annotation) = list.annotations.get(index) {
                self.text(
                    &mut bitmap,
                    annotation,
                    (candidate_end + 12.0).max(100.0),
                    (top + 24) as f32,
                    17.0,
                    TEXT,
                );
            }
        }
        let page = start / CandidateList::PAGE_SIZE + 1;
        let pages = list.items.len().div_ceil(CandidateList::PAGE_SIZE);
        self.text(
            &mut bitmap,
            &format!("{page}/{pages}  PgUp/PgDn  1-9/Enter  Esc"),
            12.0,
            (height - 10) as f32,
            14.0,
            TEXT,
        );
        Some(bitmap)
    }

    fn text(
        &self,
        bitmap: &mut Bitmap,
        text: &str,
        mut x: f32,
        baseline: f32,
        size: f32,
        color: [u8; 3],
    ) -> f32 {
        for ch in text.chars() {
            let ch = if self.font(ch).is_some() { ch } else { '?' };
            let Some(font) = self.font(ch) else { continue };
            let scaled = font.as_scaled(PxScale::from(size));
            let id = font.glyph_id(ch);
            let advance = scaled.h_advance(id);
            if x + advance > bitmap.width as f32 - 12.0 {
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
        let first = painter.render(&list, "").unwrap();
        list.selected = 1;
        let second = painter.render(&list, "").unwrap();
        assert_ne!(first.data, second.data);
        list.selected = 9;
        let last_page = painter.render(&list, "").unwrap();
        assert_eq!(first.height - last_page.height, 7 * ROW);
        assert_eq!(
            last_page.data.len(),
            (last_page.stride * last_page.height) as usize
        );
    }

    #[test]
    fn empty_or_invalid_candidates_are_never_rendered() {
        let painter = Painter::new();
        assert!(painter.render(&CandidateList::default(), "").is_none());
        let list = CandidateList {
            items: vec!["韓".into()],
            selected: 2,
            ..CandidateList::default()
        };
        assert!(painter.render(&list, "한").is_none());
    }

    #[test]
    fn missing_fonts_refuse_invisible_candidates() {
        let painter = Painter {
            korean: None,
            latin: None,
        };
        let list = CandidateList {
            items: vec!["韓".into()],
            ..CandidateList::default()
        };
        assert!(painter.render(&list, "한").is_none());
    }
}
