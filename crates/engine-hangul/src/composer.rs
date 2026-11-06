//! Syllable composition state machine.
//!
//! Holds the syllable currently being typed (leading consonant, vowel,
//! trailing consonant) and decides when it is finished and committed.

use crate::jamo::{
    cho_index, combine_final, combine_vowel, compose, is_vowel, jong_index, split_final,
    split_vowel,
};

/// The syllable under construction.
///
/// Invariant: `jong.is_some()` implies `cho.is_some() && jung.is_some()`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Composer {
    cho: Option<char>,
    jung: Option<char>,
    jong: Option<char>,
}

impl Composer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.cho.is_none() && self.jung.is_none()
    }

    /// Feed one compatibility jamo. Returns text that is now finished and
    /// must be committed (possibly empty). The new composition is available
    /// from [`Composer::preedit`].
    pub fn input(&mut self, jamo: char) -> String {
        if is_vowel(jamo) {
            self.input_vowel(jamo)
        } else if cho_index(jamo).is_some() {
            self.input_consonant(jamo)
        } else {
            // Not a jamo we can compose: finish what we have and pass it on.
            let mut out = self.flush();
            out.push(jamo);
            out
        }
    }

    fn input_vowel(&mut self, v: char) -> String {
        match (self.cho, self.jung, self.jong) {
            // Nothing yet, or only a leading consonant: the vowel joins in.
            (None, None, _) | (Some(_), None, _) => {
                self.jung = Some(v);
                String::new()
            }
            // Lone vowel or consonant+vowel: try a compound vowel.
            (_, Some(j), None) => match combine_vowel(j, v) {
                Some(c) => {
                    self.jung = Some(c);
                    String::new()
                }
                None => {
                    let out = self.flush();
                    self.jung = Some(v);
                    out
                }
            },
            // A trailing consonant moves to the next syllable.
            (Some(cho), Some(jung), Some(jong)) => {
                let (keep, moved) = match split_final(jong) {
                    Some((a, b)) => (Some(a), b),
                    None => (None, jong),
                };
                let out = compose(cho, jung, keep)
                    .map(String::from)
                    .unwrap_or_default();
                *self = Composer {
                    cho: Some(moved),
                    jung: Some(v),
                    jong: None,
                };
                out
            }
            // Unreachable by the invariant; recover by starting over.
            (None, Some(_), Some(_)) => {
                let out = self.flush();
                self.jung = Some(v);
                out
            }
        }
    }

    fn input_consonant(&mut self, c: char) -> String {
        match (self.cho, self.jung, self.jong) {
            (None, None, _) => {
                self.cho = Some(c);
                String::new()
            }
            // A lone consonant or lone vowel is finished by a consonant.
            (Some(_), None, _) | (None, Some(_), _) => {
                let out = self.flush();
                self.cho = Some(c);
                out
            }
            // Syllable without a trailing consonant: ㄸ ㅃ ㅉ cannot trail.
            (Some(_), Some(_), None) => {
                if jong_index(c).is_some() {
                    self.jong = Some(c);
                    String::new()
                } else {
                    let out = self.flush();
                    self.cho = Some(c);
                    out
                }
            }
            // Try a compound trailing consonant, otherwise start a new syllable.
            (Some(_), Some(_), Some(jong)) => match combine_final(jong, c) {
                Some(combined) => {
                    self.jong = Some(combined);
                    String::new()
                }
                None => {
                    let out = self.flush();
                    self.cho = Some(c);
                    out
                }
            },
        }
    }

    /// Remove the last jamo. Returns false if there was nothing to remove.
    pub fn backspace(&mut self) -> bool {
        if let Some(jong) = self.jong {
            self.jong = split_final(jong).map(|(a, _)| a);
        } else if let Some(jung) = self.jung {
            self.jung = split_vowel(jung).map(|(a, _)| a);
        } else if self.cho.is_some() {
            self.cho = None;
        } else {
            return false;
        }
        true
    }

    /// Remove the whole syllable under construction.
    /// Returns false if there was nothing to remove.
    pub fn backspace_syllable(&mut self) -> bool {
        let had = !self.is_empty();
        self.reset();
        had
    }

    /// The text of the syllable under construction.
    pub fn preedit(&self) -> String {
        match (self.cho, self.jung) {
            (Some(c), Some(v)) => compose(c, v, self.jong)
                .map(String::from)
                .unwrap_or_default(),
            (Some(c), None) => c.to_string(),
            (None, Some(v)) => v.to_string(),
            (None, None) => String::new(),
        }
    }

    /// Finish the current syllable and return it.
    pub fn flush(&mut self) -> String {
        let out = self.preedit();
        *self = Composer::default();
        out
    }

    /// Discard the current syllable.
    pub fn reset(&mut self) {
        *self = Composer::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Type jamo, returning (everything committed, remaining preedit).
    fn type_jamo(s: &str) -> (String, String) {
        let mut c = Composer::new();
        let mut committed = String::new();
        for j in s.chars() {
            committed.push_str(&c.input(j));
        }
        (committed, c.preedit())
    }

    #[test]
    fn composes_a_syllable() {
        assert_eq!(type_jamo("ㅎㅏㄴ"), (String::new(), "한".into()));
        assert_eq!(type_jamo("ㄱㅏ"), (String::new(), "가".into()));
    }

    #[test]
    fn finishes_syllable_on_next_consonant() {
        assert_eq!(type_jamo("ㅎㅏㄴㄱㅡㄹ"), ("한".into(), "글".into()));
    }

    #[test]
    fn moves_trailing_consonant_to_next_syllable() {
        assert_eq!(type_jamo("ㄱㅏㅂㅏ"), ("가".into(), "바".into()));
    }

    #[test]
    fn splits_compound_final_on_vowel() {
        assert_eq!(type_jamo("ㄱㅏㅂㅅㅏ"), ("갑".into(), "사".into()));
        assert_eq!(type_jamo("ㄷㅏㄹㄱㅏ"), ("달".into(), "가".into()));
    }

    #[test]
    fn compound_vowels() {
        assert_eq!(type_jamo("ㅇㅗㅏ"), (String::new(), "와".into()));
        assert_eq!(type_jamo("ㅇㅜㅓ"), (String::new(), "워".into()));
        assert_eq!(type_jamo("ㅇㅡㅣ"), (String::new(), "의".into()));
        assert_eq!(type_jamo("ㅗㅣ"), (String::new(), "ㅚ".into()));
    }

    #[test]
    fn compound_finals() {
        assert_eq!(type_jamo("ㄱㅏㅂㅅ"), (String::new(), "값".into()));
        assert_eq!(type_jamo("ㄷㅏㄹㄱ"), (String::new(), "닭".into()));
    }

    #[test]
    fn doubled_consonants_cannot_trail() {
        assert_eq!(type_jamo("ㄱㅏㄸ"), ("가".into(), "ㄸ".into()));
    }

    #[test]
    fn lone_jamo_stay_separate() {
        assert_eq!(type_jamo("ㄱㄴ"), ("ㄱ".into(), "ㄴ".into()));
        assert_eq!(type_jamo("ㅏㅏ"), ("ㅏ".into(), "ㅏ".into()));
        assert_eq!(type_jamo("ㅏㄱ"), ("ㅏ".into(), "ㄱ".into()));
    }

    #[test]
    fn non_jamo_flushes_and_passes_through() {
        let mut c = Composer::new();
        c.input('ㅎ');
        c.input('ㅏ');
        assert_eq!(c.input('!'), "하!");
        assert!(c.is_empty());
    }

    #[test]
    fn backspace_removes_one_jamo_at_a_time() {
        let mut c = Composer::new();
        for j in "ㅎㅏㄴ".chars() {
            c.input(j);
        }
        assert!(c.backspace());
        assert_eq!(c.preedit(), "하");
        assert!(c.backspace());
        assert_eq!(c.preedit(), "ㅎ");
        assert!(c.backspace());
        assert_eq!(c.preedit(), "");
        assert!(!c.backspace());
    }

    #[test]
    fn backspace_steps_through_compound_jamo() {
        let mut c = Composer::new();
        for j in "ㄱㅏㅂㅅ".chars() {
            c.input(j);
        }
        c.backspace();
        assert_eq!(c.preedit(), "갑");
        c.backspace();
        assert_eq!(c.preedit(), "가");

        let mut c = Composer::new();
        for j in "ㅇㅗㅐ".chars() {
            c.input(j);
        }
        assert_eq!(c.preedit(), "왜");
        c.backspace();
        assert_eq!(c.preedit(), "오");
    }

    #[test]
    fn backspace_syllable_clears_everything() {
        let mut c = Composer::new();
        for j in "ㄱㅏㅂㅅ".chars() {
            c.input(j);
        }
        assert!(c.backspace_syllable());
        assert!(c.is_empty());
        assert!(!c.backspace_syllable());
    }

    #[test]
    fn flush_and_reset() {
        let mut c = Composer::new();
        c.input('ㄱ');
        c.input('ㅏ');
        assert_eq!(c.flush(), "가");
        assert!(c.is_empty());
        c.input('ㄱ');
        c.reset();
        assert!(c.is_empty());
    }
}
