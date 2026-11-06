//! Translate X keysyms (as resolved by xkbcommon) into `ime-core` key events.
//!
//! Keysym values are written out as plain numbers so this module is pure and
//! easy to unit test. See `xkbcommon-keysyms.h` for the names.

use ime_core::{Key, KeyEvent, Modifiers};

const KEY_ISO_LEVEL3_SHIFT: u32 = 0xfe03;
const KEY_ISO_LEFT_TAB: u32 = 0xfe20;
const KEY_HANGUL: u32 = 0xff31;
const KEY_BACKSPACE: u32 = 0xff08;
const KEY_TAB: u32 = 0xff09;
const KEY_RETURN: u32 = 0xff0d;
const KEY_ESCAPE: u32 = 0xff1b;
const KEY_LEFT: u32 = 0xff51;
const KEY_UP: u32 = 0xff52;
const KEY_RIGHT: u32 = 0xff53;
const KEY_DOWN: u32 = 0xff54;
const KEY_MODE_SWITCH: u32 = 0xff7e;
const KEY_NUM_LOCK: u32 = 0xff7f;
const KEY_KP_ENTER: u32 = 0xff8d;
const KEY_SHIFT_L: u32 = 0xffe1;
const KEY_HYPER_R: u32 = 0xffee;
const KEY_ALT_R: u32 = 0xffea;
const KEY_SPACE: u32 = 0x20;

/// True for keys that only change modifier state (Shift, Ctrl, Alt, Super,
/// Caps Lock, ...). They must never reach the engine, or pressing Shift in the
/// middle of a syllable would end the composition.
pub fn is_modifier_keysym(keysym: u32) -> bool {
    matches!(keysym, KEY_SHIFT_L..=KEY_HYPER_R)
        || matches!(
            keysym,
            KEY_ISO_LEVEL3_SHIFT | KEY_MODE_SWITCH | KEY_NUM_LOCK
        )
}

/// True for the keys that switch between English and the current language:
/// the Hangul key and Right Alt (which is AltGr on some layouts).
pub fn is_language_toggle(keysym: u32) -> bool {
    matches!(keysym, KEY_HANGUL | KEY_ALT_R | KEY_ISO_LEVEL3_SHIFT)
}

/// Convert a keysym plus modifier state to an engine key event.
pub fn translate(keysym: u32, modifiers: Modifiers) -> KeyEvent {
    let key = match keysym {
        KEY_BACKSPACE => Key::Backspace,
        KEY_TAB | KEY_ISO_LEFT_TAB => Key::Tab,
        KEY_RETURN | KEY_KP_ENTER => Key::Enter,
        KEY_ESCAPE => Key::Escape,
        KEY_SPACE => Key::Space,
        KEY_LEFT => Key::Left,
        KEY_UP => Key::Up,
        KEY_RIGHT => Key::Right,
        KEY_DOWN => Key::Down,
        // Printable ASCII: the keysym already has Shift applied ('R' vs 'r').
        0x21..=0x7e => Key::Char(char::from(keysym as u8)),
        _ => Key::Other,
    };
    KeyEvent { key, modifiers }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: Modifiers = Modifiers::NONE;

    #[test]
    fn printable_ascii_keeps_case() {
        assert_eq!(translate(0x72, NONE).key, Key::Char('r'));
        assert_eq!(translate(0x52, NONE).key, Key::Char('R'));
        assert_eq!(translate(0x31, NONE).key, Key::Char('1'));
        assert_eq!(translate(0x2e, NONE).key, Key::Char('.'));
    }

    #[test]
    fn named_keys() {
        assert_eq!(translate(0xff08, NONE).key, Key::Backspace);
        assert_eq!(translate(0x20, NONE).key, Key::Space);
        assert_eq!(translate(0xff0d, NONE).key, Key::Enter);
        assert_eq!(translate(0xff8d, NONE).key, Key::Enter);
        assert_eq!(translate(0xff1b, NONE).key, Key::Escape);
        assert_eq!(translate(0xff51, NONE).key, Key::Left);
        assert_eq!(translate(0xff54, NONE).key, Key::Down);
    }

    #[test]
    fn unknown_keys_are_other() {
        assert_eq!(translate(0xffbe, NONE).key, Key::Other); // F1
        assert_eq!(translate(0x00e9, NONE).key, Key::Other); // é
    }

    #[test]
    fn modifiers_are_carried_through() {
        let ctrl = Modifiers { ctrl: true, ..NONE };
        let e = translate(0x63, ctrl);
        assert_eq!(e.key, Key::Char('c'));
        assert!(e.modifiers.ctrl);
    }

    #[test]
    fn detects_modifier_keys() {
        for k in [0xffe1, 0xffe2, 0xffe3, 0xffe5, 0xffe9, 0xffeb, 0xfe03] {
            assert!(is_modifier_keysym(k), "{k:#x}");
        }
        assert!(!is_modifier_keysym(0x72));
        assert!(!is_modifier_keysym(0xff08));
    }

    #[test]
    fn detects_language_toggle_keys() {
        assert!(is_language_toggle(0xff31)); // Hangul
        assert!(is_language_toggle(0xffea)); // Alt_R
        assert!(!is_language_toggle(0xffe9)); // Alt_L
        assert!(!is_language_toggle(0x20));
    }
}
