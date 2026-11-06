//! Hangul compatibility jamo tables and syllable composition.
//!
//! All functions work on compatibility jamo (U+3131..U+3163), which is also
//! what we show as preedit for a lone consonant or vowel.

/// Leading consonants in Unicode syllable order.
const CHO: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ',
    'ㅌ', 'ㅍ', 'ㅎ',
];

/// Vowels in Unicode syllable order.
const JUNG: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ',
    'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];

/// Trailing consonants in Unicode syllable order; index 0 means "none".
const JONG: [char; 28] = [
    '\0', 'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ',
    'ㅁ', 'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

/// Compound vowels: (first, second, result).
const VOWEL_COMBOS: [(char, char, char); 7] = [
    ('ㅗ', 'ㅏ', 'ㅘ'),
    ('ㅗ', 'ㅐ', 'ㅙ'),
    ('ㅗ', 'ㅣ', 'ㅚ'),
    ('ㅜ', 'ㅓ', 'ㅝ'),
    ('ㅜ', 'ㅔ', 'ㅞ'),
    ('ㅜ', 'ㅣ', 'ㅟ'),
    ('ㅡ', 'ㅣ', 'ㅢ'),
];

/// Compound trailing consonants: (first, second, result).
const FINAL_COMBOS: [(char, char, char); 11] = [
    ('ㄱ', 'ㅅ', 'ㄳ'),
    ('ㄴ', 'ㅈ', 'ㄵ'),
    ('ㄴ', 'ㅎ', 'ㄶ'),
    ('ㄹ', 'ㄱ', 'ㄺ'),
    ('ㄹ', 'ㅁ', 'ㄻ'),
    ('ㄹ', 'ㅂ', 'ㄼ'),
    ('ㄹ', 'ㅅ', 'ㄽ'),
    ('ㄹ', 'ㅌ', 'ㄾ'),
    ('ㄹ', 'ㅍ', 'ㄿ'),
    ('ㄹ', 'ㅎ', 'ㅀ'),
    ('ㅂ', 'ㅅ', 'ㅄ'),
];

/// Index of `c` as a leading consonant (0..19).
pub fn cho_index(c: char) -> Option<u32> {
    CHO.iter().position(|&x| x == c).map(|i| i as u32)
}

/// Index of `c` as a vowel (0..21).
pub fn jung_index(c: char) -> Option<u32> {
    JUNG.iter().position(|&x| x == c).map(|i| i as u32)
}

/// Index of `c` as a trailing consonant (1..28); `None` if it cannot be one.
pub fn jong_index(c: char) -> Option<u32> {
    JONG.iter()
        .skip(1)
        .position(|&x| x == c)
        .map(|i| i as u32 + 1)
}

pub fn is_vowel(c: char) -> bool {
    jung_index(c).is_some()
}

/// True for any consonant jamo usable as a leading or trailing consonant.
pub fn is_consonant(c: char) -> bool {
    cho_index(c).is_some() || jong_index(c).is_some()
}

/// Compose a syllable. `jong` is `None` for no trailing consonant.
/// Returns `None` if a jamo is not valid in its position.
pub fn compose(cho: char, jung: char, jong: Option<char>) -> Option<char> {
    let l = cho_index(cho)?;
    let v = jung_index(jung)?;
    let t = match jong {
        Some(c) => jong_index(c)?,
        None => 0,
    };
    char::from_u32(0xAC00 + (l * 21 + v) * 28 + t)
}

/// Combine two vowels into a compound vowel (ㅗ+ㅏ → ㅘ).
pub fn combine_vowel(a: char, b: char) -> Option<char> {
    VOWEL_COMBOS
        .iter()
        .find(|&&(x, y, _)| x == a && y == b)
        .map(|&(_, _, r)| r)
}

/// Split a compound vowel (ㅘ → ㅗ,ㅏ).
pub fn split_vowel(c: char) -> Option<(char, char)> {
    VOWEL_COMBOS
        .iter()
        .find(|&&(_, _, r)| r == c)
        .map(|&(a, b, _)| (a, b))
}

/// Combine two consonants into a compound trailing consonant (ㄱ+ㅅ → ㄳ).
pub fn combine_final(a: char, b: char) -> Option<char> {
    FINAL_COMBOS
        .iter()
        .find(|&&(x, y, _)| x == a && y == b)
        .map(|&(_, _, r)| r)
}

/// Split a compound trailing consonant (ㄳ → ㄱ,ㅅ).
pub fn split_final(c: char) -> Option<(char, char)> {
    FINAL_COMBOS
        .iter()
        .find(|&&(_, _, r)| r == c)
        .map(|&(a, b, _)| (a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_basic_syllables() {
        assert_eq!(compose('ㅎ', 'ㅏ', Some('ㄴ')), Some('한'));
        assert_eq!(compose('ㄱ', 'ㅡ', Some('ㄹ')), Some('글'));
        assert_eq!(compose('ㄱ', 'ㅏ', None), Some('가'));
        assert_eq!(compose('ㅎ', 'ㅣ', Some('ㅎ')), Some('힣'));
    }

    #[test]
    fn composes_compound_jamo() {
        assert_eq!(compose('ㄱ', 'ㅏ', Some('ㅄ')), Some('값'));
        assert_eq!(compose('ㅇ', 'ㅝ', Some('ㄶ')), Some('웒'));
    }

    #[test]
    fn rejects_invalid_positions() {
        // ㄸ ㅃ ㅉ cannot be trailing consonants.
        assert_eq!(compose('ㄱ', 'ㅏ', Some('ㄸ')), None);
        // A vowel cannot lead.
        assert_eq!(compose('ㅏ', 'ㅏ', None), None);
        assert_eq!(compose('ㄱ', 'ㄱ', None), None);
    }

    #[test]
    fn classifies_jamo() {
        assert!(is_vowel('ㅏ') && !is_vowel('ㄱ'));
        assert!(is_consonant('ㄱ') && is_consonant('ㄳ') && !is_consonant('ㅏ'));
        assert_eq!(cho_index('ㄸ'), Some(4));
        assert_eq!(jong_index('ㄸ'), None);
        assert_eq!(jong_index('ㄱ'), Some(1));
        assert_eq!(jong_index('ㅎ'), Some(27));
    }

    #[test]
    fn combines_and_splits_vowels() {
        assert_eq!(combine_vowel('ㅗ', 'ㅏ'), Some('ㅘ'));
        assert_eq!(combine_vowel('ㅡ', 'ㅣ'), Some('ㅢ'));
        assert_eq!(combine_vowel('ㅏ', 'ㅣ'), None);
        for &(a, b, r) in &VOWEL_COMBOS {
            assert_eq!(split_vowel(r), Some((a, b)));
        }
    }

    #[test]
    fn combines_and_splits_finals() {
        assert_eq!(combine_final('ㄱ', 'ㅅ'), Some('ㄳ'));
        assert_eq!(combine_final('ㄹ', 'ㅎ'), Some('ㅀ'));
        assert_eq!(combine_final('ㄱ', 'ㄱ'), None);
        for &(a, b, r) in &FINAL_COMBOS {
            assert_eq!(split_final(r), Some((a, b)));
            assert!(jong_index(r).is_some());
        }
    }
}
