//! Finding a font that can draw the indicator glyphs.
//!
//! The Latin "A" works with nearly any font. "한", "あ" and "中" need a CJK
//! font, which most Linux desktops have as Noto Sans CJK (or Source Han Sans).
//! We look for a well-known family first, so the same glyph looks right for
//! its language (Japanese "あ" and Chinese "中" are drawn from the matching
//! regional font), and fall back to any installed font that has the glyph.
//! If nothing has it, the caller draws an empty box. This module never panics
//! on missing or broken fonts.
//!
//! **Partly portable.** `fontdb` and `ab_glyph` work on every OS, but the family names
//! below are the ones Linux desktops have. Windows and macOS ship their own CJK
//! fonts, so a frontend there needs its own list (`docs/platforms.md`).

use ab_glyph::{Font, FontVec, VariableFont};
use fontdb::{Database, Family, Query, Weight};

/// Weight requested from variable fonts for the cursor-adjacent box.
const INDICATOR_WEIGHT: f32 = 600.0;
/// Weight requested for small task bar icons so strokes stay crisp at 16-32 px.
const TRAY_WEIGHT: f32 = 700.0;

const KOREAN: &[&str] = &[
    "Noto Sans CJK KR",
    "Noto Sans KR",
    "Source Han Sans K",
    "Source Han Sans KR",
    "Noto Sans Mono CJK KR",
    "NanumGothic",
];
const JAPANESE: &[&str] = &[
    "Noto Sans CJK JP",
    "Noto Sans JP",
    "Source Han Sans",
    "Source Han Sans JP",
    "Noto Sans Mono CJK JP",
];
const CHINESE: &[&str] = &[
    "Noto Sans CJK SC",
    "Noto Sans SC",
    "Source Han Sans SC",
    "Source Han Sans CN",
    "Noto Sans Mono CJK SC",
    "WenQuanYi Micro Hei",
];
const LATIN: &[&str] = &["Noto Sans", "DejaVu Sans", "Liberation Sans", "Cantarell"];

/// Font families to try for `ch`, best first.
pub fn preferred_families(ch: char) -> &'static [&'static str] {
    match ch {
        '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' | '\u{AC00}'..='\u{D7AF}' => KOREAN,
        '\u{3040}'..='\u{30FF}' => JAPANESE,
        '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' => CHINESE,
        _ => LATIN,
    }
}

/// The installed fonts, loaded once.
pub struct Fonts {
    db: Database,
}

impl Fonts {
    /// Scan the system font directories. Takes tens of milliseconds, so do it
    /// once and keep the result.
    pub fn load_system() -> Self {
        let mut db = Database::new();
        db.load_system_fonts();
        Self { db }
    }

    /// A font collection with nothing in it.
    pub fn empty() -> Self {
        Self {
            db: Database::new(),
        }
    }

    /// Number of installed font faces.
    pub fn len(&self) -> usize {
        self.db.len()
    }

    pub fn is_empty(&self) -> bool {
        self.db.is_empty()
    }

    /// A font that has a real (not ".notdef") glyph for `ch`, if any.
    pub fn font_for(&self, ch: char) -> Option<FontVec> {
        self.font_with_weight(ch, Weight::NORMAL, INDICATOR_WEIGHT)
    }

    /// A bolder font for small task bar icons (16–32 px), so the letter stays
    /// crisp and legible next to other tray icons.
    pub fn font_for_tray(&self, ch: char) -> Option<FontVec> {
        self.font_with_weight(ch, Weight::BOLD, TRAY_WEIGHT)
    }

    fn font_with_weight(&self, ch: char, weight: Weight, var_weight: f32) -> Option<FontVec> {
        for name in preferred_families(ch) {
            let query = Query {
                families: &[Family::Name(name)],
                weight,
                ..Query::default()
            };
            if let Some(id) = self.db.query(&query)
                && let Some(font) = self.load(id, ch, var_weight)
            {
                return Some(font);
            }
        }
        // Any font with the glyph: slower, but only reached when no known
        // family is installed.
        self.db
            .faces()
            .filter(|face| !face.post_script_name.is_empty() || !face.families.is_empty())
            .find_map(|face| self.load(face.id, ch, var_weight))
    }

    fn load(&self, id: fontdb::ID, ch: char, var_weight: f32) -> Option<FontVec> {
        let mut font = self
            .db
            .with_face_data(id, |data, index| {
                FontVec::try_from_vec_and_index(data.to_vec(), index).ok()
            })
            .flatten()?;
        // Variable fonts (Noto Sans CJK is one on Fedora) start at their
        // default instance, which can be hair-thin at this size. Light text on
        // a dark box wants a heavier weight; static fonts have no such axis
        // and ignore the request.
        let _ = font.set_variation(b"wght", var_weight);
        (font.glyph_id(ch).0 != 0).then_some(font)
    }
}

/// Convenience: load the system fonts and look for one with `ch`.
pub fn find_font(ch: char) -> Option<FontVec> {
    Fonts::load_system().font_for(ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regional_families_come_first() {
        assert!(preferred_families('한')[0].ends_with("KR"));
        assert!(preferred_families('あ')[0].ends_with("JP"));
        assert!(preferred_families('中')[0].ends_with("SC"));
        assert_eq!(preferred_families('A')[0], "Noto Sans");
    }

    #[test]
    fn no_fonts_means_no_font_and_no_panic() {
        let fonts = Fonts::empty();
        assert!(fonts.is_empty());
        for ch in ['A', '한', 'あ', '中'] {
            assert!(fonts.font_for(ch).is_none());
        }
    }

    #[test]
    fn garbage_font_data_is_ignored() {
        let mut db = Database::new();
        db.load_font_data(b"this is not a font".to_vec());
        let fonts = Fonts { db };
        assert!(fonts.font_for('A').is_none());
    }

    #[test]
    fn system_fonts_cover_latin_when_any_are_installed() {
        let fonts = Fonts::load_system();
        if fonts.is_empty() {
            eprintln!("SKIPPED: no system fonts installed; nothing to check");
            return;
        }
        let font = fonts.font_for('A');
        assert!(font.is_some(), "{} fonts, none has 'A'", fonts.len());
    }

    #[test]
    fn cjk_glyphs_are_found_when_noto_cjk_is_installed() {
        let fonts = Fonts::load_system();
        let has_noto_cjk = fonts.db.faces().any(|f| {
            f.families
                .iter()
                .any(|(name, _)| name.starts_with("Noto Sans CJK"))
        });
        if !has_noto_cjk {
            eprintln!("SKIPPED: Noto Sans CJK is not installed; CJK discovery not checked");
            return;
        }
        for ch in ['한', 'あ', '中'] {
            assert!(fonts.font_for(ch).is_some(), "no font for {ch}");
        }
    }
}
