//! The Japanese engine (kana input only for now).

use crate::kana::to_katakana;
use crate::romaji::Romaji;
use ime_core::{EngineOutput, InputEngine, Key, KeyEvent, Preedit};

/// Which script the engine writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KanaMode {
    #[default]
    Hiragana,
    Katakana,
}

/// Punctuation keys that are part of the romaji table.
const PUNCTUATION: &str = "-,./[]~?!";

/// Romaji input to kana.
///
/// While composing: Enter commits (and is consumed), Space commits and is
/// forwarded, Escape cancels, Backspace edits. For keys it does not consume
/// (`handled == false`), `commit` holds finished text that the frontend must
/// insert before forwarding the key.
#[derive(Debug, Default)]
pub struct JapaneseEngine {
    romaji: Romaji,
    mode: KanaMode,
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
        EngineOutput {
            commit,
            preedit: Preedit::new(self.render(self.romaji.preedit())),
            candidates: None,
            handled,
        }
    }

    fn finish(&mut self) -> String {
        let text = self.romaji.flush();
        self.render(text)
    }
}

impl InputEngine for JapaneseEngine {
    fn process_key(&mut self, event: KeyEvent) -> EngineOutput {
        // Shortcuts are not text: finish and pass through.
        if event.modifiers.any() {
            let commit = self.finish();
            return self.output(commit, false);
        }

        let composing = !self.romaji.is_empty();
        match event.key {
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
    fn space_commits_and_is_forwarded() {
        let mut e = JapaneseEngine::new();
        type_keys(&mut e, "kana");
        let out = e.process_key(KeyEvent::new(Key::Space));
        assert_eq!(out.commit, "かな");
        assert!(!out.handled);
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
}
