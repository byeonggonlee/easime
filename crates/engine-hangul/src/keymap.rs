//! Dubeolsik (standard Korean 2-set) keyboard layout.

/// Map a US QWERTY character (Shift applied) to a compatibility jamo.
///
/// Shift only changes the five doubled consonants (ㅃㅉㄸㄲㅆ) and the two
/// double-letter vowels (ㅒㅖ); every other uppercase letter maps like its
/// lowercase one.
pub fn dubeolsik(c: char) -> Option<char> {
    Some(match c {
        'q' => 'ㅂ',
        'w' => 'ㅈ',
        'e' => 'ㄷ',
        'r' => 'ㄱ',
        't' => 'ㅅ',
        'y' | 'Y' => 'ㅛ',
        'u' | 'U' => 'ㅕ',
        'i' | 'I' => 'ㅑ',
        'o' => 'ㅐ',
        'p' => 'ㅔ',
        'a' | 'A' => 'ㅁ',
        's' | 'S' => 'ㄴ',
        'd' | 'D' => 'ㅇ',
        'f' | 'F' => 'ㄹ',
        'g' | 'G' => 'ㅎ',
        'h' | 'H' => 'ㅗ',
        'j' | 'J' => 'ㅓ',
        'k' | 'K' => 'ㅏ',
        'l' | 'L' => 'ㅣ',
        'z' | 'Z' => 'ㅋ',
        'x' | 'X' => 'ㅌ',
        'c' | 'C' => 'ㅊ',
        'v' | 'V' => 'ㅍ',
        'b' | 'B' => 'ㅠ',
        'n' | 'N' => 'ㅜ',
        'm' | 'M' => 'ㅡ',
        'Q' => 'ㅃ',
        'W' => 'ㅉ',
        'E' => 'ㄸ',
        'R' => 'ㄲ',
        'T' => 'ㅆ',
        'O' => 'ㅒ',
        'P' => 'ㅖ',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jamo::{is_consonant, is_vowel};

    #[test]
    fn maps_home_row() {
        let row: String = "asdfghjkl".chars().filter_map(dubeolsik).collect();
        assert_eq!(row, "ㅁㄴㅇㄹㅎㅗㅓㅏㅣ");
    }

    #[test]
    fn shift_makes_doubled_jamo() {
        let shifted: String = "QWERTOP".chars().filter_map(dubeolsik).collect();
        assert_eq!(shifted, "ㅃㅉㄸㄲㅆㅒㅖ");
    }

    #[test]
    fn shift_is_ignored_for_other_letters() {
        for c in "yuiasdfghjklzxcvbnm".chars() {
            assert_eq!(dubeolsik(c), dubeolsik(c.to_ascii_uppercase()), "{c}");
        }
    }

    #[test]
    fn every_letter_maps_to_a_jamo() {
        for c in ('a'..='z').chain('A'..='Z') {
            let j = dubeolsik(c).unwrap_or_else(|| panic!("no mapping for {c}"));
            assert!(is_vowel(j) || is_consonant(j), "{c} -> {j}");
        }
    }

    #[test]
    fn non_letters_are_unmapped() {
        for c in ['1', ' ', '.', '[', '/'] {
            assert_eq!(dubeolsik(c), None);
        }
    }
}
