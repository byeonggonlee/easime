//! OS-independent key events.
//!
//! Frontends translate platform key codes (keysym, virtual key, ...) into
//! these types. `Key::Char` carries the character the key produces on a
//! US QWERTY layout (with Shift applied), which is what Hangul Dubeolsik and
//! romaji engines map from.

/// A logical key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// A printable key, as produced on a US QWERTY layout (Shift applied).
    Char(char),
    Backspace,
    Enter,
    Space,
    Escape,
    Tab,
    Left,
    Right,
    Up,
    Down,
    /// Any key the engines do not care about.
    Other,
}

/// Modifier keys held during a key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers {
        ctrl: false,
        alt: false,
        meta: false,
    };

    /// True if any non-Shift modifier is held (such keys are shortcuts, not text).
    pub fn any(self) -> bool {
        self.ctrl || self.alt || self.meta
    }
}

/// A key press delivered to an engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl KeyEvent {
    pub fn new(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers::NONE,
        }
    }

    pub fn ch(c: char) -> Self {
        Self::new(Key::Char(c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_any() {
        assert!(!Modifiers::NONE.any());
        assert!(
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            }
            .any()
        );
    }

    #[test]
    fn char_event_has_no_modifiers() {
        let e = KeyEvent::ch('r');
        assert_eq!(e.key, Key::Char('r'));
        assert!(!e.modifiers.any());
    }
}
