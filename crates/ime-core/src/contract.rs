//! Promises every [`InputEngine`] makes, as checks an engine's own tests call.
//!
//! This is test support, kept here so that a new engine (Chinese, ...) is held
//! to the same promises as the first two without copying the checks.

use crate::engine::{EngineOutput, InputEngine};
use crate::key::{Key, KeyEvent, Modifiers};

/// Every key worth asking about: all printable ASCII characters, the named keys,
/// each plain, with Shift's effect already in the character, and with Ctrl or
/// Alt held.
pub fn keys_worth_trying() -> Vec<KeyEvent> {
    let mut keys: Vec<Key> = (0x20u8..0x7f).map(|b| Key::Char(char::from(b))).collect();
    keys.extend([
        Key::Backspace,
        Key::Enter,
        Key::Space,
        Key::Escape,
        Key::Tab,
        Key::Left,
        Key::Right,
        Key::Up,
        Key::Down,
        Key::Hanja,
        Key::PageUp,
        Key::PageDown,
        Key::Other,
    ]);
    let held = [
        Modifiers::NONE,
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
        Modifiers {
            alt: true,
            ..Modifiers::NONE
        },
    ];
    keys.iter()
        .flat_map(|&key| held.map(|modifiers| KeyEvent { key, modifiers }))
        .collect()
}

fn replay(
    make: &dyn Fn() -> Box<dyn InputEngine>,
    prefix: &[KeyEvent],
) -> (Box<dyn InputEngine>, EngineOutput) {
    let mut engine = make();
    let mut last = EngineOutput::default();
    for &key in prefix {
        last = engine.process_key(key);
    }
    (engine, last)
}

/// The promise of [`InputEngine::wants_key`]: asking changes nothing, and a
/// key it says no to is a key `process_key` would leave alone (nothing
/// committed, the composition and candidates unchanged, not handled).
///
/// `make` builds a fresh engine; `prefixes` are key sequences that bring it to
/// the states worth checking (idle, composing, converting, ...). Every key of
/// [`keys_worth_trying`] is tried in every state.
///
/// # Panics
/// With the state and key that broke the promise.
pub fn check_wants_key(make: &dyn Fn() -> Box<dyn InputEngine>, prefixes: &[Vec<KeyEvent>]) {
    for prefix in prefixes {
        for key in keys_worth_trying() {
            let (mut asked, before) = replay(make, prefix);
            let wants = asked.wants_key(key);
            // Asking again, and asking about other keys, must not matter.
            for other in keys_worth_trying().into_iter().step_by(7) {
                let _ = asked.wants_key(other);
            }
            let (mut plain, _) = replay(make, prefix);
            let expected = plain.process_key(key);
            let actual = asked.process_key(key);
            assert_eq!(
                actual, expected,
                "asking wants_key changed the result of {key:?} after {prefix:?}"
            );
            if !wants {
                assert!(
                    actual.commit.is_empty()
                        && !actual.handled
                        && actual.preedit == before.preedit
                        && actual.candidates == before.candidates,
                    "wants_key said no to {key:?} after {prefix:?}, but process_key gave {actual:?} (before: {before:?})"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Preedit;

    /// Eats the letter `a` into a composition and nothing else.
    #[derive(Default)]
    struct Toy {
        text: String,
        /// What `wants_key` answers for a letter while idle (the lie under test).
        honest: bool,
    }

    impl InputEngine for Toy {
        fn process_key(&mut self, key: KeyEvent) -> EngineOutput {
            let handled = matches!(key.key, Key::Char('a')) && !key.modifiers.any();
            if handled {
                self.text.push('a');
            }
            EngineOutput {
                preedit: Preedit::new(self.text.clone()),
                handled,
                ..EngineOutput::default()
            }
        }

        fn wants_key(&self, key: KeyEvent) -> bool {
            self.honest && matches!(key.key, Key::Char('a')) && !key.modifiers.any()
        }

        fn reset(&mut self) {
            self.text.clear();
        }

        fn flush(&mut self) -> String {
            std::mem::take(&mut self.text)
        }
    }

    #[test]
    fn an_honest_engine_passes() {
        check_wants_key(
            &|| {
                Box::new(Toy {
                    honest: true,
                    ..Toy::default()
                })
            },
            &[vec![], vec![KeyEvent::ch('a')]],
        );
    }

    #[test]
    #[should_panic(expected = "wants_key said no")]
    fn an_engine_that_hides_a_key_it_takes_is_caught() {
        check_wants_key(&|| Box::<Toy>::default(), &[vec![]]);
    }

    #[test]
    fn the_keys_cover_text_named_keys_and_shortcuts() {
        let keys = keys_worth_trying();
        assert!(keys.contains(&KeyEvent::ch('a')));
        assert!(keys.contains(&KeyEvent::new(Key::Enter)));
        assert!(keys.iter().any(|k| k.modifiers.ctrl));
        assert!(keys.iter().any(|k| k.modifiers.alt));
    }
}
