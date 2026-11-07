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

/// The name of `language` in its own script, for a menu or a tooltip: people
/// pick a language by what it looks like to them, not by a translation
/// (DESIGN.md 4.1). The two Chinese scripts are told apart by their names.
pub fn language_native_name(language: Language) -> &'static str {
    match language {
        Language::English => "English",
        Language::Korean => "한국어",
        Language::Japanese => "日本語",
        Language::ChineseSimplified => "简体中文",
        Language::ChineseTraditional => "繁體中文",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_named_in_their_own_script() {
        assert_eq!(language_native_name(Language::English), "English");
        assert_eq!(language_native_name(Language::Korean), "한국어");
        assert_eq!(language_native_name(Language::Japanese), "日本語");
        assert_eq!(
            language_native_name(Language::ChineseSimplified),
            "简体中文"
        );
        assert_eq!(
            language_native_name(Language::ChineseTraditional),
            "繁體中文"
        );
    }

    #[test]
    fn each_language_has_its_glyph() {
        assert_eq!(language_glyph(Language::English), 'A');
        assert_eq!(language_glyph(Language::Korean), '한');
        assert_eq!(language_glyph(Language::Japanese), 'あ');
        assert_eq!(language_glyph(Language::ChineseSimplified), '中');
        assert_eq!(language_glyph(Language::ChineseTraditional), '中');
    }
}
