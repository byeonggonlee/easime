//! Input languages and the cycle the toggle key walks through.

use serde::{Deserialize, Serialize};

/// A language the input method can type in. English means "no composition":
/// keys pass straight to the application.
///
/// Simplified and Traditional Chinese are separate entries, like the separate
/// Chinese keyboards of Windows and macOS: the script is part of what the user
/// picks and what the switch key cycles through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    English,
    Korean,
    Japanese,
    ChineseSimplified,
    ChineseTraditional,
}

/// What the plain switch key does (decision of 2026-11-09).
///
/// The other way to move on, [`LanguageSwitcher::advance`] (Shift with the
/// switch key), always goes round the cycle whatever this says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchMode {
    /// Between English and the language last typed in: from English back to
    /// that language, from any other language to English. Mixing English with
    /// one language is one press whichever way, however many languages are on.
    #[default]
    ToggleLast,
    /// To the next language of the cycle, English, Korean, Japanese, English...
    Cycle,
}

impl SwitchMode {
    pub fn is_default(&self) -> bool {
        *self == SwitchMode::default()
    }
}

/// The order languages are visited in: English first, then the enabled
/// languages in the configured order. [`advance`](Self::advance) moves to the
/// next one and wraps around; [`toggle`](Self::toggle) is what the plain switch
/// key does, which depends on the [`SwitchMode`].
///
/// The language last typed in is not remembered across runs: a new switcher
/// starts on the first enabled language, which is also the "last" one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageSwitcher {
    cycle: Vec<Language>,
    index: usize,
    mode: SwitchMode,
    /// Index in `cycle` of the last language other than English that was
    /// active (0 when there is none).
    last: usize,
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
        Self {
            cycle,
            index,
            mode: SwitchMode::default(),
            last: index,
        }
    }

    /// The same switcher with the plain switch key doing `mode`.
    pub fn with_mode(mut self, mode: SwitchMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn mode(&self) -> SwitchMode {
        self.mode
    }

    /// The language the plain key would go to from English: the last one that
    /// was active, or `None` when only English is on.
    pub fn last(&self) -> Option<Language> {
        (self.last != 0).then(|| self.cycle[self.last])
    }

    /// Make `language` the one the plain key returns to from English, without
    /// switching (kept across a change of settings). Ignored if it is English
    /// or not in the cycle.
    pub fn remember(&mut self, language: Language) {
        if language != Language::English
            && let Some(index) = self.cycle.iter().position(|&l| l == language)
        {
            self.last = index;
        }
    }

    fn go_to(&mut self, index: usize) {
        self.index = index;
        if index != 0 {
            self.last = index;
        }
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
                self.go_to(index);
                true
            }
            None => false,
        }
    }

    /// Move to the next language of the cycle and return it (Shift with the
    /// switch key, and the plain key in [`SwitchMode::Cycle`]).
    pub fn advance(&mut self) -> Language {
        self.go_to((self.index + 1) % self.cycle.len());
        self.current()
    }

    /// What the plain switch key does: the next language in
    /// [`SwitchMode::Cycle`]; in [`SwitchMode::ToggleLast`] English when
    /// another language is on, else the last language (or English again when
    /// only English is on). Returns the language that is on now.
    pub fn toggle(&mut self) -> Language {
        match self.mode {
            SwitchMode::Cycle => self.advance(),
            SwitchMode::ToggleLast => {
                let target = if self.index == 0 { self.last } else { 0 };
                self.go_to(target);
                self.current()
            }
        }
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
        let mut s = LanguageSwitcher::new(&[Korean, Japanese, ChineseSimplified]);
        assert_eq!(s.advance(), Japanese);
        assert_eq!(s.advance(), ChineseSimplified);
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
        assert!(!s.select(ChineseTraditional));
        assert_eq!(s.current(), Korean);
    }

    #[test]
    fn the_two_chinese_scripts_are_separate_entries() {
        let mut s = LanguageSwitcher::new(&[ChineseSimplified, ChineseTraditional]);
        assert_eq!(
            s.languages(),
            &[English, ChineseSimplified, ChineseTraditional]
        );
        assert_eq!(s.advance(), ChineseTraditional);
        assert_eq!(s.advance(), English);
    }

    #[test]
    fn languages_are_written_in_snake_case() {
        #[derive(Deserialize)]
        struct Wrapper {
            language: Language,
        }
        let read = |name: &str| -> Result<Language, toml::de::Error> {
            toml::from_str::<Wrapper>(&format!("language = \"{name}\"")).map(|w| w.language)
        };
        assert_eq!(read("chinese_simplified").unwrap(), ChineseSimplified);
        assert_eq!(read("chinese_traditional").unwrap(), ChineseTraditional);
        assert!(read("chinese").is_err());
    }

    #[test]
    fn drops_duplicates_and_english() {
        let s = LanguageSwitcher::new(&[English, Korean, Korean, Japanese]);
        assert_eq!(s.languages(), &[English, Korean, Japanese]);
    }

    #[test]
    fn the_plain_key_goes_between_english_and_the_last_language() {
        let mut s = LanguageSwitcher::new(&[Korean, Japanese]);
        assert_eq!(s.mode(), SwitchMode::ToggleLast);
        assert_eq!(s.current(), Korean);
        assert_eq!(s.toggle(), English);
        assert_eq!(s.toggle(), Korean, "back to the one it came from");
        assert_eq!(s.toggle(), English);
        assert_eq!(s.toggle(), Korean);
    }

    #[test]
    fn the_next_language_is_what_the_other_gesture_goes_to() {
        let mut s = LanguageSwitcher::new(&[Korean, Japanese]);
        assert_eq!(s.advance(), Japanese);
        assert_eq!(s.last(), Some(Japanese), "the language last typed in");
        assert_eq!(s.toggle(), English);
        assert_eq!(s.toggle(), Japanese, "not Korean: Japanese came last");
        assert_eq!(s.advance(), English);
        assert_eq!(s.last(), Some(Japanese), "English does not count");
        assert_eq!(s.toggle(), Japanese);
    }

    #[test]
    fn cycle_mode_keeps_the_old_behaviour() {
        let mut s = LanguageSwitcher::new(&[Korean, Japanese]).with_mode(SwitchMode::Cycle);
        assert_eq!(s.toggle(), Japanese);
        assert_eq!(s.toggle(), English);
        assert_eq!(s.toggle(), Korean);
    }

    #[test]
    fn choosing_a_language_makes_it_the_last_one() {
        let mut s = LanguageSwitcher::new(&[Korean, Japanese]);
        assert!(s.select(Japanese));
        assert_eq!(s.toggle(), English);
        assert_eq!(s.toggle(), Japanese);
        assert!(s.select(English));
        assert_eq!(s.last(), Some(Japanese), "choosing English changes nothing");
        assert_eq!(s.toggle(), Japanese);
    }

    #[test]
    fn with_only_english_nothing_moves_in_either_mode() {
        let mut s = LanguageSwitcher::new(&[]);
        assert_eq!(s.last(), None);
        assert_eq!(s.toggle(), English);
        assert_eq!(s.advance(), English);
    }

    #[test]
    fn a_remembered_language_survives_a_new_switcher() {
        let mut s = LanguageSwitcher::new(&[Korean, Japanese]);
        s.remember(Japanese);
        s.remember(English);
        s.remember(ChineseSimplified);
        assert_eq!(s.last(), Some(Japanese), "English and unknown are ignored");
        assert_eq!(s.current(), Korean, "remembering does not switch");
    }

    #[test]
    fn toggling_the_last_language_is_the_default() {
        assert!(SwitchMode::default().is_default());
        assert!(!SwitchMode::Cycle.is_default());
    }
}
