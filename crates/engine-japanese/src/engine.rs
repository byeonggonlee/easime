//! The Japanese engine (kana input only for now).

use crate::dictionary::{Dictionary, EmptyDictionary};
use crate::kana::to_katakana;
use crate::romaji::Romaji;
use ime_core::{CandidateList, EngineOutput, InputEngine, Key, KeyEvent, Preedit};

/// Which script the engine writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KanaMode {
    #[default]
    Hiragana,
    Katakana,
}

/// Punctuation keys that are part of the romaji table.
const PUNCTUATION: &str = "-,./[]~?!";

/// A conversion in progress: the reading and its candidates.
#[derive(Debug)]
struct Conversion {
    reading: String,
    candidates: Vec<String>,
    selected: usize,
}

impl Conversion {
    fn current(&self) -> &str {
        &self.candidates[self.selected]
    }
}

/// Romaji input to kana, with dictionary conversion.
///
/// While composing: Enter commits the kana (and is consumed), Space starts a
/// conversion, Escape cancels, Backspace edits. While converting: Space/Down
/// and Up move through the candidates, Enter commits the chosen one, Escape or
/// Backspace go back to the kana, a digit 1-9 picks a candidate, and any other
/// key commits the chosen candidate first.
///
/// For keys it does not consume (`handled == false`), `commit` holds finished
/// text that the frontend must insert before forwarding the key.
pub struct JapaneseEngine {
    romaji: Romaji,
    mode: KanaMode,
    dictionary: Box<dyn Dictionary>,
    conversion: Option<Conversion>,
}

impl Default for JapaneseEngine {
    fn default() -> Self {
        Self {
            romaji: Romaji::new(),
            mode: KanaMode::default(),
            dictionary: Box::new(EmptyDictionary),
            conversion: None,
        }
    }
}

impl JapaneseEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_mode(mode: KanaMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    /// Use `dictionary` for kana to kanji conversion.
    pub fn with_dictionary(dictionary: Box<dyn Dictionary>) -> Self {
        Self {
            dictionary,
            ..Self::default()
        }
    }

    pub fn set_mode(&mut self, mode: KanaMode) {
        self.mode = mode;
    }

    fn render(&self, text: String) -> String {
        match self.mode {
            KanaMode::Hiragana => text,
            KanaMode::Katakana => to_katakana(&text),
        }
    }

    fn output(&self, commit: String, handled: bool) -> EngineOutput {
        if let Some(conv) = &self.conversion {
            let text = conv.current().to_string();
            let mut preedit = Preedit::new(text.clone());
            preedit.highlight = Some(0..text.chars().count());
            return EngineOutput {
                commit,
                preedit,
                candidates: Some(CandidateList {
                    items: conv.candidates.clone(),
                    selected: conv.selected,
                    annotations: Vec::new(),
                }),
                handled,
            };
        }
        EngineOutput {
            commit,
            preedit: Preedit::new(self.render(self.romaji.preedit())),
            candidates: None,
            handled,
        }
    }

    /// Finish whatever is in progress and return the text to commit.
    fn finish(&mut self) -> String {
        if let Some(conv) = self.conversion.take() {
            return conv.current().to_string();
        }
        let text = self.romaji.flush();
        self.render(text)
    }

    /// Begin converting the kana typed so far.
    fn start_conversion(&mut self) {
        let reading = self.romaji.flush();
        let mut candidates = self.dictionary.lookup(&reading);
        // Kana is always an option, after what the dictionary suggests.
        for fallback in [reading.clone(), to_katakana(&reading)] {
            if !candidates.contains(&fallback) {
                candidates.push(fallback);
            }
        }
        self.conversion = Some(Conversion {
            reading,
            candidates,
            selected: 0,
        });
    }

    /// Give up the conversion and return to editing the kana.
    fn cancel_conversion(&mut self) {
        if let Some(conv) = self.conversion.take() {
            self.romaji.restore(&conv.reading);
        }
    }

    fn move_selection(&mut self, forward: bool) {
        if let Some(conv) = &mut self.conversion {
            let n = conv.candidates.len();
            conv.selected = if forward {
                (conv.selected + 1) % n
            } else {
                (conv.selected + n - 1) % n
            };
        }
    }

    fn process_converting(&mut self, event: KeyEvent) -> EngineOutput {
        match event.key {
            Key::Space | Key::Down => {
                self.move_selection(true);
                self.output(String::new(), true)
            }
            Key::Up => {
                self.move_selection(false);
                self.output(String::new(), true)
            }
            Key::Left | Key::Right => self.output(String::new(), true),
            Key::Enter => {
                let commit = self.finish();
                self.output(commit, true)
            }
            Key::Escape | Key::Backspace => {
                self.cancel_conversion();
                self.output(String::new(), true)
            }
            Key::Char(c @ '1'..='9') => {
                let index = c as usize - '1' as usize;
                let count = self.conversion.as_ref().map_or(0, |c| c.candidates.len());
                if index < count {
                    if let Some(conv) = &mut self.conversion {
                        conv.selected = index;
                    }
                    let commit = self.finish();
                    self.output(commit, true)
                } else {
                    // No such candidate: treat it like any other key.
                    let commit = self.finish();
                    self.output(commit, false)
                }
            }
            Key::Char(c) if c.is_ascii_alphabetic() => {
                // Typing on: commit the choice and start the next word.
                let commit = self.finish();
                self.romaji.push(c);
                self.output(commit, true)
            }
            _ => {
                let commit = self.finish();
                self.output(commit, false)
            }
        }
    }
}

impl InputEngine for JapaneseEngine {
    fn process_key(&mut self, event: KeyEvent) -> EngineOutput {
        // Shortcuts are not text: finish and pass through.
        if event.modifiers.any() {
            let commit = self.finish();
            return self.output(commit, false);
        }

        if self.conversion.is_some() {
            return self.process_converting(event);
        }

        let composing = !self.romaji.is_empty();
        match event.key {
            Key::Space if composing => {
                self.start_conversion();
                self.output(String::new(), true)
            }
            Key::Char(c) if c.is_ascii_alphabetic() || PUNCTUATION.contains(c) => {
                self.romaji.push(c);
                self.output(String::new(), true)
            }
            Key::Backspace => {
                let handled = self.romaji.backspace();
                self.output(String::new(), handled)
            }
            Key::Enter if composing => {
                let commit = self.finish();
                self.output(commit, true)
            }
            Key::Escape if composing => {
                self.romaji.reset();
                self.output(String::new(), true)
            }
            _ => {
                let commit = self.finish();
                self.output(commit, false)
            }
        }
    }

    fn reset(&mut self) {
        self.conversion = None;
        self.romaji.reset();
    }

    fn flush(&mut self) -> String {
        self.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ime_core::Modifiers;

    /// Type keys and return (all commits, final preedit).
    fn type_keys(e: &mut JapaneseEngine, keys: &str) -> (String, String) {
        let mut committed = String::new();
        let mut preedit = String::new();
        for c in keys.chars() {
            let out = e.process_key(KeyEvent::ch(c));
            committed.push_str(&out.commit);
            preedit = out.preedit.text;
        }
        (committed, preedit)
    }

    #[test]
    fn composes_kana_in_the_preedit() {
        let mut e = JapaneseEngine::new();
        assert_eq!(type_keys(&mut e, "nihon"), (String::new(), "にほn".into()));
    }

    #[test]
    fn enter_commits_and_is_consumed() {
        let mut e = JapaneseEngine::new();
        type_keys(&mut e, "nihongo");
        let out = e.process_key(KeyEvent::new(Key::Enter));
        assert_eq!(out.commit, "にほんご");
        assert!(out.handled);
        assert!(out.preedit.is_empty());
    }

    #[test]
    fn enter_without_composition_is_forwarded() {
        let mut e = JapaneseEngine::new();
        let out = e.process_key(KeyEvent::new(Key::Enter));
        assert!(!out.handled);
        assert!(out.commit.is_empty());
    }

    #[test]
    fn space_without_composition_is_forwarded() {
        let mut e = JapaneseEngine::new();
        let out = e.process_key(KeyEvent::new(Key::Space));
        assert!(!out.handled);
        assert!(out.commit.is_empty());
        assert!(out.candidates.is_none());
    }

    #[test]
    fn trailing_n_becomes_n_on_commit() {
        let mut e = JapaneseEngine::new();
        type_keys(&mut e, "han");
        assert_eq!(e.process_key(KeyEvent::new(Key::Enter)).commit, "はん");
    }

    #[test]
    fn escape_cancels_the_composition() {
        let mut e = JapaneseEngine::new();
        type_keys(&mut e, "kana");
        let out = e.process_key(KeyEvent::new(Key::Escape));
        assert!(out.handled && out.commit.is_empty() && out.preedit.is_empty());
        // With nothing composed, Escape goes to the application.
        assert!(!e.process_key(KeyEvent::new(Key::Escape)).handled);
    }

    #[test]
    fn backspace_edits_then_passes_through() {
        let mut e = JapaneseEngine::new();
        type_keys(&mut e, "ka");
        let out = e.process_key(KeyEvent::new(Key::Backspace));
        assert!(out.handled && out.preedit.is_empty());
        assert!(!e.process_key(KeyEvent::new(Key::Backspace)).handled);
    }

    #[test]
    fn digits_flush_and_pass_through() {
        let mut e = JapaneseEngine::new();
        type_keys(&mut e, "ka");
        let out = e.process_key(KeyEvent::ch('1'));
        assert_eq!(out.commit, "か");
        assert!(!out.handled);
    }

    #[test]
    fn punctuation_joins_the_composition() {
        let mut e = JapaneseEngine::new();
        assert_eq!(type_keys(&mut e, "a,"), (String::new(), "あ、".into()));
    }

    #[test]
    fn shortcuts_pass_through() {
        let mut e = JapaneseEngine::new();
        type_keys(&mut e, "ka");
        let ctrl_c = KeyEvent {
            key: Key::Char('c'),
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        };
        let out = e.process_key(ctrl_c);
        assert_eq!(out.commit, "か");
        assert!(!out.handled);
    }

    #[test]
    fn katakana_mode_converts_preedit_and_commit() {
        let mut e = JapaneseEngine::with_mode(KanaMode::Katakana);
        let (_, preedit) = type_keys(&mut e, "nihon");
        assert_eq!(preedit, "ニホn");
        assert_eq!(e.flush(), "ニホン");
    }

    #[test]
    fn reset_drops_the_composition() {
        let mut e = JapaneseEngine::new();
        type_keys(&mut e, "ka");
        e.reset();
        assert_eq!(e.flush(), "");
    }

    // ---- conversion -------------------------------------------------------

    use crate::dictionary::MemoryDictionary;

    fn engine_with_dictionary() -> JapaneseEngine {
        let (dict, _) =
            MemoryDictionary::parse("にほん\t日本,二本\nにほんご\t日本語\nかな\t仮名,かな\n");
        JapaneseEngine::with_dictionary(Box::new(dict))
    }

    fn key(e: &mut JapaneseEngine, key: Key) -> EngineOutput {
        e.process_key(KeyEvent::new(key))
    }

    #[test]
    fn space_starts_a_conversion_with_dictionary_then_kana_candidates() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "nihon");
        let out = key(&mut e, Key::Space);
        assert!(out.handled && out.commit.is_empty());
        assert_eq!(out.preedit.text, "日本");
        assert_eq!(out.preedit.highlight, Some(0..2));
        let list = out.candidates.unwrap();
        assert_eq!(list.items, ["日本", "二本", "にほん", "ニホン"]);
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn unknown_words_offer_hiragana_then_katakana() {
        let mut e = JapaneseEngine::new();
        type_keys(&mut e, "kuruma");
        let out = key(&mut e, Key::Space);
        assert_eq!(out.preedit.text, "くるま");
        assert_eq!(out.candidates.unwrap().items, ["くるま", "クルマ"]);
    }

    #[test]
    fn a_dictionary_candidate_equal_to_the_reading_is_not_repeated() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "kana");
        let out = key(&mut e, Key::Space);
        assert_eq!(out.candidates.unwrap().items, ["仮名", "かな", "カナ"]);
    }

    #[test]
    fn trailing_n_is_part_of_the_reading() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "nihon");
        // The waiting "n" must count as ん before the lookup.
        assert_eq!(key(&mut e, Key::Space).preedit.text, "日本");
    }

    #[test]
    fn space_and_down_move_forward_and_wrap_up_moves_back() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "nihon");
        key(&mut e, Key::Space);
        assert_eq!(key(&mut e, Key::Space).preedit.text, "二本");
        assert_eq!(key(&mut e, Key::Down).preedit.text, "にほん");
        assert_eq!(key(&mut e, Key::Down).preedit.text, "ニホン");
        assert_eq!(key(&mut e, Key::Space).preedit.text, "日本"); // wrapped
        assert_eq!(key(&mut e, Key::Up).preedit.text, "ニホン"); // wrapped back
        assert_eq!(key(&mut e, Key::Up).candidates.unwrap().selected, 2);
    }

    #[test]
    fn enter_commits_the_selected_candidate() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "nihon");
        key(&mut e, Key::Space);
        key(&mut e, Key::Space);
        let out = key(&mut e, Key::Enter);
        assert_eq!(out.commit, "二本");
        assert!(out.handled && out.preedit.is_empty() && out.candidates.is_none());
    }

    #[test]
    fn escape_returns_to_the_kana() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "nihon");
        key(&mut e, Key::Space);
        let out = key(&mut e, Key::Escape);
        assert!(out.handled && out.candidates.is_none());
        assert_eq!(out.preedit.text, "にほん");
        // And editing continues from there.
        assert_eq!(type_keys(&mut e, "go"), (String::new(), "にほんご".into()));
        assert_eq!(key(&mut e, Key::Enter).commit, "にほんご");
    }

    #[test]
    fn backspace_also_returns_to_the_kana_without_deleting() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "nihon");
        key(&mut e, Key::Space);
        let out = key(&mut e, Key::Backspace);
        assert!(out.handled);
        assert_eq!(out.preedit.text, "にほん");
    }

    #[test]
    fn typing_a_letter_commits_the_choice_and_starts_a_new_word() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "nihon");
        key(&mut e, Key::Space);
        let out = e.process_key(KeyEvent::ch('k'));
        assert_eq!(out.commit, "日本");
        assert!(out.handled);
        assert_eq!(out.preedit.text, "k");
        assert!(out.candidates.is_none());
    }

    #[test]
    fn a_digit_picks_that_candidate() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "nihon");
        key(&mut e, Key::Space);
        let out = e.process_key(KeyEvent::ch('2'));
        assert_eq!(out.commit, "二本");
        assert!(out.handled);
    }

    #[test]
    fn a_digit_without_such_a_candidate_commits_and_passes_through() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "nihon");
        key(&mut e, Key::Space);
        let out = e.process_key(KeyEvent::ch('9'));
        assert_eq!(out.commit, "日本");
        assert!(!out.handled);
    }

    #[test]
    fn other_keys_commit_the_choice_and_pass_through() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "nihon");
        key(&mut e, Key::Space);
        let out = key(&mut e, Key::Tab);
        assert_eq!(out.commit, "日本");
        assert!(!out.handled);
        assert!(out.preedit.is_empty());
    }

    #[test]
    fn shortcuts_commit_the_choice_and_pass_through() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "nihon");
        key(&mut e, Key::Space);
        let ctrl = KeyEvent {
            key: Key::Char('c'),
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        };
        let out = e.process_key(ctrl);
        assert_eq!(out.commit, "日本");
        assert!(!out.handled);
    }

    #[test]
    fn flush_commits_the_chosen_candidate() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "nihon");
        key(&mut e, Key::Space);
        key(&mut e, Key::Space);
        assert_eq!(e.flush(), "二本");
        assert_eq!(e.flush(), "");
    }

    #[test]
    fn reset_cancels_a_conversion() {
        let mut e = engine_with_dictionary();
        type_keys(&mut e, "nihon");
        key(&mut e, Key::Space);
        e.reset();
        assert_eq!(e.flush(), "");
        let out = key(&mut e, Key::Enter);
        assert!(!out.handled, "nothing left to commit");
    }
}
