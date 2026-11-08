//! Incremental romaji to hiragana conversion.

use crate::table::{has_longer, lookup};

/// Converts typed romaji to hiragana one character at a time.
///
/// `kana` holds the finished hiragana; `pending` holds romaji that may still
/// become part of a longer sequence (for example `k` waiting for `a`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Romaji {
    kana: String,
    pending: String,
}

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'i' | 'u' | 'e' | 'o')
}

/// A letter that can start a double consonant (っ): any consonant except n.
fn can_double(c: char) -> bool {
    c.is_ascii_lowercase() && !is_vowel(c) && c != 'n'
}

impl Romaji {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.kana.is_empty() && self.pending.is_empty()
    }

    /// Feed one character (letters are case-insensitive).
    pub fn push(&mut self, c: char) {
        let c = c.to_ascii_lowercase();
        self.pending.push(c);
        self.resolve();
    }

    /// Turn as much of `pending` into kana as can be decided now.
    fn resolve(&mut self) {
        while !self.pending.is_empty() {
            if let Some(kana) = lookup(&self.pending) {
                if has_longer(&self.pending) {
                    return; // wait: a longer entry may still match
                }
                self.kana.push_str(kana);
                self.pending.clear();
                return;
            }
            if has_longer(&self.pending) {
                return; // a valid prefix, wait for more input
            }

            // No entry matches. Decide what the first character means.
            let mut chars = self.pending.chars();
            let first = chars.next().unwrap();
            let second = chars.next();
            let rest = &self.pending[first.len_utf8()..];
            match second {
                // "n" before a consonant is ん ("kanta" -> かんた).
                Some(s) if first == 'n' && !is_vowel(s) && s != 'y' => {
                    self.kana.push('ん');
                    self.pending = rest.to_string();
                }
                // Doubled consonant is っ ("kka" -> っか). Hepburn "tch" counts
                // too ("matcha" -> まっちゃ).
                Some(s) if (first == s && can_double(first)) || (first == 't' && s == 'c') => {
                    self.kana.push('っ');
                    self.pending = rest.to_string();
                }
                // Not romaji we know: keep the letter as typed.
                _ => {
                    self.kana.push(first);
                    self.pending = rest.to_string();
                }
            }
        }
    }

    /// Remove the last typed unit. Returns false if there was nothing.
    pub fn backspace(&mut self) -> bool {
        if self.pending.pop().is_some() {
            return true;
        }
        self.kana.pop().is_some()
    }

    /// What to show as the composition: finished kana plus waiting romaji.
    pub fn preedit(&self) -> String {
        format!("{}{}", self.kana, self.pending)
    }

    /// Finish: a lone waiting `n` becomes ん, other leftovers stay as typed.
    pub fn flush(&mut self) -> String {
        if self.pending == "n" {
            self.kana.push('ん');
        } else {
            self.kana.push_str(&self.pending);
        }
        self.pending.clear();
        std::mem::take(&mut self.kana)
    }

    /// Start over with already converted kana (for example after cancelling a
    /// conversion), with nothing waiting.
    pub fn restore(&mut self, kana: &str) {
        self.kana = kana.to_string();
        self.pending.clear();
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kana(input: &str) -> String {
        let mut r = Romaji::new();
        for c in input.chars() {
            r.push(c);
        }
        r.flush()
    }

    #[test]
    fn basic_syllables() {
        assert_eq!(kana("aiueo"), "あいうえお");
        assert_eq!(kana("kakikukeko"), "かきくけこ");
        assert_eq!(kana("sushi"), "すし");
        assert_eq!(kana("chizu"), "ちず");
        assert_eq!(kana("tsukue"), "つくえ");
    }

    #[test]
    fn common_words() {
        assert_eq!(kana("nihongo"), "にほんご");
        assert_eq!(kana("sayonara"), "さよなら");
        assert_eq!(kana("arigatou"), "ありがとう");
        assert_eq!(kana("konnnichiha"), "こんにちは");
        assert_eq!(kana("kon'nichiha"), "こんにちは");
        assert_eq!(kana("tokyo"), "ときょ");
        assert_eq!(kana("kyouto"), "きょうと");
        assert_eq!(kana("shinbun"), "しんぶん");
        assert_eq!(kana("tanni"), "たんい");
        assert_eq!(kana("gennin"), "げんいん");
        assert_eq!(kana("shinnyou"), "しんよう");
    }

    #[test]
    fn small_tsu_from_double_consonants() {
        assert_eq!(kana("kitte"), "きって");
        assert_eq!(kana("gakkou"), "がっこう");
        assert_eq!(kana("zasshi"), "ざっし");
        assert_eq!(kana("chotto"), "ちょっと");
        assert_eq!(kana("matcha"), "まっちゃ");
    }

    #[test]
    fn n_handling() {
        assert_eq!(kana("onnna"), "おんな");
        assert_eq!(kana("on'na"), "おんな");
        assert_eq!(kana("kanji"), "かんじ");
        assert_eq!(kana("kenya"), "けにゃ");
        assert_eq!(kana("kennya"), "けんや");
        assert_eq!(kana("ken'ya"), "けんや");
        assert_eq!(kana("nya"), "にゃ");
        assert_eq!(kana("nna"), "んあ");
        assert_eq!(kana("nnna"), "んな");
        assert_eq!(kana("n'a"), "んあ");
        assert_eq!(kana("han"), "はん"); // trailing n becomes ん on flush
        assert_eq!(kana("nn"), "ん");
        assert_eq!(kana("kannnyou"), "かんにょう");
    }

    #[test]
    fn small_kana_and_extended_sounds() {
        assert_eq!(kana("xtu"), "っ");
        assert_eq!(kana("ltsu"), "っ");
        assert_eq!(kana("fairu"), "ふぁいる");
        assert_eq!(kana("thi"), "てぃ");
        assert_eq!(kana("wo"), "を");
        assert_eq!(kana("vu"), "ゔ");
    }

    #[test]
    fn punctuation_and_long_vowel() {
        assert_eq!(kana("ra-men"), "らーめん");
        assert_eq!(kana("ha,ha."), "は、は。");
        assert_eq!(kana("[a]"), "「あ」");
    }

    #[test]
    fn uppercase_is_treated_as_lowercase() {
        assert_eq!(kana("KaNa"), "かな");
    }

    #[test]
    fn unknown_sequences_stay_as_typed() {
        assert_eq!(kana("kq"), "kq");
        assert_eq!(kana("bx"), "bx");
    }

    #[test]
    fn preedit_shows_waiting_romaji() {
        let mut r = Romaji::new();
        r.push('k');
        assert_eq!(r.preedit(), "k");
        r.push('y');
        assert_eq!(r.preedit(), "ky");
        r.push('a');
        assert_eq!(r.preedit(), "きゃ");
        r.push('n');
        assert_eq!(r.preedit(), "きゃn");
    }

    #[test]
    fn backspace_removes_waiting_romaji_then_kana() {
        let mut r = Romaji::new();
        for c in "kak".chars() {
            r.push(c);
        }
        assert!(r.backspace());
        assert_eq!(r.preedit(), "か");
        assert!(r.backspace());
        assert_eq!(r.preedit(), "");
        assert!(!r.backspace());
        assert!(r.is_empty());
    }

    #[test]
    fn nn_then_backspace_removes_finished_n() {
        let mut r = Romaji::new();
        for c in "nn".chars() {
            r.push(c);
        }
        r.backspace();
        r.push('a');
        assert_eq!(r.flush(), "あ");
    }

    #[test]
    fn restore_resumes_editing_from_kana() {
        let mut r = Romaji::new();
        r.restore("にほん");
        assert_eq!(r.preedit(), "にほん");
        r.push('g');
        r.push('o');
        assert_eq!(r.flush(), "にほんご");
    }

    #[test]
    fn flush_and_reset() {
        let mut r = Romaji::new();
        r.push('k');
        r.push('a');
        assert_eq!(r.flush(), "か");
        assert!(r.is_empty());
        r.push('k');
        r.reset();
        assert!(r.is_empty());
    }
}
