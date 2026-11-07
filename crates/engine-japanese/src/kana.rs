//! Kana script conversion.

/// Convert hiragana to katakana. Other characters are left as they are.
pub fn to_katakana(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            // ぁ..ゖ map to ァ..ヶ; ゔ (U+3094) maps to ヴ (U+30F4).
            '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            // Iteration marks ゝ ゞ -> ヽ ヾ
            '\u{309d}' | '\u{309e}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_hiragana() {
        assert_eq!(to_katakana("にほんご"), "ニホンゴ");
        assert_eq!(to_katakana("きょう"), "キョウ");
        assert_eq!(to_katakana("ゔぁ"), "ヴァ");
        assert_eq!(to_katakana("っ"), "ッ");
    }

    #[test]
    fn leaves_other_characters() {
        assert_eq!(to_katakana("らーめん。"), "ラーメン。");
        assert_eq!(to_katakana("abc123"), "abc123");
        assert_eq!(to_katakana("カナ"), "カナ");
    }
}
