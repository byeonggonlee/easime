//! The Dubeolsik Hangul engine.

use crate::composer::Composer;
use crate::keymap::dubeolsik;
use ime_core::{EngineOutput, InputEngine, Key, KeyEvent, Preedit};

/// How much Backspace removes while a syllable is being composed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackspaceMode {
    /// One jamo at a time: 한 → 하 → ㅎ → (nothing).
    #[default]
    Jamo,
    /// The whole syllable: 한 → (nothing).
    Syllable,
}

/// Hangul input engine using the Dubeolsik layout.
///
/// For keys it does not consume (`handled == false`), the returned `commit`
/// holds any finished text; the frontend must insert it **before** forwarding
/// the key to the application.
#[derive(Debug, Default)]
pub struct HangulEngine {
    composer: Composer,
    backspace_mode: BackspaceMode,
}

impl HangulEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_backspace_mode(mode: BackspaceMode) -> Self {
        Self {
            backspace_mode: mode,
            ..Self::default()
        }
    }

    pub fn set_backspace_mode(&mut self, mode: BackspaceMode) {
        self.backspace_mode = mode;
    }

    fn output(&self, commit: String, handled: bool) -> EngineOutput {
        EngineOutput {
            commit,
            preedit: Preedit::new(self.composer.preedit()),
            candidates: None,
            handled,
        }
    }
}

impl InputEngine for HangulEngine {
    fn process_key(&mut self, event: KeyEvent) -> EngineOutput {
        // Shortcuts (Ctrl+C, Alt+F, ...) are not text: finish and pass through.
        if event.modifiers.any() {
            let commit = self.composer.flush();
            return self.output(commit, false);
        }

        match event.key {
            Key::Char(c) => match dubeolsik(c) {
                Some(jamo) => {
                    let commit = self.composer.input(jamo);
                    self.output(commit, true)
                }
                None => {
                    let commit = self.composer.flush();
                    self.output(commit, false)
                }
            },
            Key::Backspace => {
                let handled = match self.backspace_mode {
                    BackspaceMode::Jamo => self.composer.backspace(),
                    BackspaceMode::Syllable => self.composer.backspace_syllable(),
                };
                self.output(String::new(), handled)
            }
            _ => {
                let commit = self.composer.flush();
                self.output(commit, false)
            }
        }
    }

    fn reset(&mut self) {
        self.composer.reset();
    }

    fn flush(&mut self) -> String {
        self.composer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ime_core::Modifiers;

    /// Type a string of QWERTY keys, collecting commits; returns (committed, preedit).
    fn type_keys(engine: &mut HangulEngine, keys: &str) -> (String, String) {
        let mut committed = String::new();
        let mut preedit = String::new();
        for c in keys.chars() {
            let out = engine.process_key(KeyEvent::ch(c));
            committed.push_str(&out.commit);
            preedit = out.preedit.text;
        }
        (committed, preedit)
    }

    #[test]
    fn types_hangul() {
        let mut e = HangulEngine::new();
        // 한글 → "gksrmf"
        assert_eq!(type_keys(&mut e, "gksrmf"), ("한".into(), "글".into()));
        assert_eq!(e.flush(), "글");
    }

    #[test]
    fn types_doubled_consonant_with_shift() {
        let mut e = HangulEngine::new();
        // 까치: ㄲ=R ㅏ=k ㅊ=c ㅣ=l
        assert_eq!(type_keys(&mut e, "Rkcl"), ("까".into(), "치".into()));
    }

    #[test]
    fn space_commits_and_is_forwarded() {
        let mut e = HangulEngine::new();
        type_keys(&mut e, "gk");
        let out = e.process_key(KeyEvent::new(Key::Space));
        assert_eq!(out.commit, "하");
        assert!(!out.handled);
        assert!(out.preedit.is_empty());
    }

    #[test]
    fn digits_and_punctuation_pass_through() {
        let mut e = HangulEngine::new();
        type_keys(&mut e, "gk");
        let out = e.process_key(KeyEvent::ch('1'));
        assert_eq!(out.commit, "하");
        assert!(!out.handled);
    }

    #[test]
    fn modifier_shortcuts_pass_through() {
        let mut e = HangulEngine::new();
        type_keys(&mut e, "gk");
        let ctrl_c = KeyEvent {
            key: Key::Char('c'),
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        };
        let out = e.process_key(ctrl_c);
        assert_eq!(out.commit, "하");
        assert!(!out.handled);
    }

    #[test]
    fn backspace_edits_preedit_then_passes_through() {
        let mut e = HangulEngine::new();
        type_keys(&mut e, "gks");
        let out = e.process_key(KeyEvent::new(Key::Backspace));
        assert!(out.handled);
        assert_eq!(out.preedit.text, "하");
        e.process_key(KeyEvent::new(Key::Backspace));
        e.process_key(KeyEvent::new(Key::Backspace));
        // Nothing left to edit: the application should handle Backspace.
        let out = e.process_key(KeyEvent::new(Key::Backspace));
        assert!(!out.handled);
    }

    #[test]
    fn syllable_backspace_mode_removes_whole_syllable() {
        let mut e = HangulEngine::with_backspace_mode(BackspaceMode::Syllable);
        type_keys(&mut e, "gks");
        let out = e.process_key(KeyEvent::new(Key::Backspace));
        assert!(out.handled);
        assert!(out.preedit.is_empty());
        assert!(!e.process_key(KeyEvent::new(Key::Backspace)).handled);
    }

    #[test]
    fn reset_drops_composition() {
        let mut e = HangulEngine::new();
        type_keys(&mut e, "gk");
        e.reset();
        assert_eq!(e.flush(), "");
    }
}
