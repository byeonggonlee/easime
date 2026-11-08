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
    conversion: Option<ime_core::CandidateList>,
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
        let mut preedit = Preedit::new(self.composer.preedit());
        if self.conversion.is_some() {
            preedit.highlight = Some(0..preedit.cursor);
        }
        EngineOutput {
            commit,
            preedit,
            candidates: self.conversion.clone(),
            handled,
        }
    }

    fn commit_candidate(&mut self) -> EngineOutput {
        let list = self.conversion.take().expect("active conversion");
        self.composer.reset();
        self.output(list.items[list.selected].clone(), true)
    }

    fn convert_key(&mut self, key: Key) -> Option<EngineOutput> {
        let list = self.conversion.as_mut()?;
        let count = list.items.len();
        let page_size = ime_core::CandidateList::PAGE_SIZE;
        match key {
            Key::Down | Key::Space => list.selected = (list.selected + 1) % count,
            Key::Up => list.selected = (list.selected + count - 1) % count,
            Key::PageDown => {
                list.selected = ((list.selected / page_size + 1) * page_size).min(count - 1);
            }
            Key::PageUp => {
                list.selected = (list.selected / page_size).saturating_sub(1) * page_size;
            }
            Key::Enter => return Some(self.commit_candidate()),
            Key::Char(c @ '1'..='9') => {
                let index = list.selected / page_size * page_size + (c as usize - '1' as usize);
                if index < count {
                    list.selected = index;
                    return Some(self.commit_candidate());
                }
            }
            Key::Escape | Key::Backspace => self.conversion = None,
            Key::Hanja => {}
            _ => {
                // Only explicit selection commits Hanja. Continue ordinary
                // typing with the original Hangul composition intact.
                self.conversion = None;
                return None;
            }
        }
        Some(self.output(String::new(), true))
    }
}

impl InputEngine for HangulEngine {
    fn process_key(&mut self, event: KeyEvent) -> EngineOutput {
        // Shortcuts (Ctrl+C, Alt+F, ...) are not text: finish and pass through.
        if event.modifiers.any() {
            self.conversion = None;
            let commit = self.composer.flush();
            return self.output(commit, false);
        }

        if let Some(output) = self.convert_key(event.key) {
            return output;
        }

        match event.key {
            Key::Hanja => {
                let reading = self.composer.preedit();
                self.conversion = crate::hanja::lookup(&reading);
                self.output(String::new(), !reading.is_empty())
            }
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

    fn wants_key(&self, event: KeyEvent) -> bool {
        // Anything in progress is finished or edited by whatever key comes.
        if self.conversion.is_some() || !self.composer.is_empty() {
            return true;
        }
        // Otherwise only a key that starts a syllable does anything.
        !event.modifiers.any() && matches!(event.key, Key::Char(c) if dubeolsik(c).is_some())
    }

    fn reset(&mut self) {
        self.conversion = None;
        self.composer.reset();
    }

    fn flush(&mut self) -> String {
        self.conversion = None;
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
    fn wants_key_keeps_its_promise_in_every_state() {
        let keys = |text: &str| -> Vec<KeyEvent> { text.chars().map(KeyEvent::ch).collect() };
        let mut converting = keys("gk");
        converting.push(KeyEvent::new(Key::Hanja));
        ime_core::contract::check_wants_key(
            &|| Box::new(HangulEngine::new()),
            &[
                vec![],
                keys("g"),
                keys("gk"),
                keys("gks"),
                keys("gksr"),
                keys("gksrmf"),
                converting,
                vec![KeyEvent::ch('1')],
            ],
        );
    }

    #[test]
    fn idle_only_the_letters_of_the_layout_are_wanted() {
        let e = HangulEngine::new();
        assert!(e.wants_key(KeyEvent::ch('g')));
        assert!(!e.wants_key(KeyEvent::ch('1')));
        assert!(!e.wants_key(KeyEvent::new(Key::Space)));
        assert!(!e.wants_key(KeyEvent::new(Key::Backspace)));
        let mut composing = HangulEngine::new();
        composing.process_key(KeyEvent::ch('g'));
        assert!(
            composing.wants_key(KeyEvent::new(Key::Space)),
            "to finish the syllable first"
        );
        assert!(composing.wants_key(KeyEvent::new(Key::Backspace)));
    }

    #[test]
    fn reset_drops_composition() {
        let mut e = HangulEngine::new();
        type_keys(&mut e, "gk");
        e.reset();
        assert_eq!(e.flush(), "");
    }
}
