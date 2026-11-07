//! The letter that stands for a language where there is room for one letter
//! only: the box next to the text cursor, a tray icon, a menu bar.
//!
//! Where it is drawn, and in which font, is the frontend's business (a CJK
//! glyph needs a font with that glyph, in the shape of its language).

use ime_core::Language;

/// The glyph shown for `language`.
///
/// Both Chinese scripts show `中` for now; whether the box should tell them
/// apart is still open (docs/decisions.md).
pub fn language_glyph(language: Language) -> char {
    match language {
        Language::English => 'A',
        Language::Korean => '한',
        Language::Japanese => 'あ',
        Language::ChineseSimplified | Language::ChineseTraditional => '中',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_language_has_its_glyph() {
        assert_eq!(language_glyph(Language::English), 'A');
        assert_eq!(language_glyph(Language::Korean), '한');
        assert_eq!(language_glyph(Language::Japanese), 'あ');
        assert_eq!(language_glyph(Language::ChineseSimplified), '中');
        assert_eq!(language_glyph(Language::ChineseTraditional), '中');
    }
}
