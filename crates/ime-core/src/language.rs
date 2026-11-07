//! Input languages and the cycle the toggle key walks through.

use serde::{Deserialize, Serialize};

/// A language the input method can type in. English means "no composition":
/// keys pass straight to the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    English,
    Korean,
    Japanese,
    Chinese,
}

/// The order languages are visited in: English first, then the enabled
/// languages in the configured order. Each toggle moves to the next one and
/// wraps around.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageSwitcher {
    cycle: Vec<Language>,
    index: usize,
}

impl LanguageSwitcher {
    /// Build the cycle from the enabled languages. Duplicates and English
    /// entries are dropped. Starts on the first enabled language, or English
    /// if there is none.
    pub fn new(enabled: &[Language]) -> Self {
        let mut cycle = vec![Language::English];
        for &lang in enabled {
            if !cycle.contains(&lang) {
                cycle.push(lang);
            }
        }
        let index = if cycle.len() > 1 { 1 } else { 0 };
        Self { cycle, index }
    }

    pub fn current(&self) -> Language {
        self.cycle[self.index]
    }

    /// Languages in cycle order, English first.
    pub fn languages(&self) -> &[Language] {
        &self.cycle
    }

    /// Switch to `language` if it is part of the cycle. Returns false (and
    /// changes nothing) otherwise.
    pub fn select(&mut self, language: Language) -> bool {
        match self.cycle.iter().position(|&l| l == language) {
            Some(index) => {
                self.index = index;
                true
            }
            None => false,
        }
    }

    /// Move to the next language and return it.
    pub fn advance(&mut self) -> Language {
        self.index = (self.index + 1) % self.cycle.len();
        self.current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Language::*;

    #[test]
    fn starts_on_the_first_enabled_language() {
        assert_eq!(LanguageSwitcher::new(&[Korean, Japanese]).current(), Korean);
    }

    #[test]
    fn cycles_english_then_enabled_languages_in_order() {
        let mut s = LanguageSwitcher::new(&[Korean, Japanese, Chinese]);
        assert_eq!(s.advance(), Japanese);
        assert_eq!(s.advance(), Chinese);
        assert_eq!(s.advance(), English);
        assert_eq!(s.advance(), Korean);
    }

    #[test]
    fn single_language_toggles_with_english() {
        let mut s = LanguageSwitcher::new(&[Korean]);
        assert_eq!(s.advance(), English);
        assert_eq!(s.advance(), Korean);
    }

    #[test]
    fn no_languages_stays_on_english() {
        let mut s = LanguageSwitcher::new(&[]);
        assert_eq!(s.current(), English);
        assert_eq!(s.advance(), English);
    }

    #[test]
    fn select_jumps_to_a_language_in_the_cycle() {
        let mut s = LanguageSwitcher::new(&[Korean, Japanese]);
        assert!(s.select(Japanese));
        assert_eq!(s.current(), Japanese);
        assert!(s.select(English));
        assert_eq!(s.current(), English);
    }

    #[test]
    fn select_ignores_languages_outside_the_cycle() {
        let mut s = LanguageSwitcher::new(&[Korean]);
        assert!(!s.select(Chinese));
        assert_eq!(s.current(), Korean);
    }

    #[test]
    fn drops_duplicates_and_english() {
        let s = LanguageSwitcher::new(&[English, Korean, Korean, Japanese]);
        assert_eq!(s.languages(), &[English, Korean, Japanese]);
    }
}
