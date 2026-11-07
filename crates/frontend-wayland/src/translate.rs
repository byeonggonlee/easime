//! Translate X keysyms (as resolved by xkbcommon) into `ime-core` key events.
//!
//! Keysym values are written out as plain numbers so this module is pure and
//! easy to unit test. See `xkbcommon-keysyms.h` for the names.

use ime_core::{Key, KeyEvent, Modifiers};

const KEY_ISO_LEVEL3_SHIFT: u32 = 0xfe03;
const KEY_ISO_LEFT_TAB: u32 = 0xfe20;
pub(crate) const KEY_HANGUL: u32 = 0xff31;
const KEY_BACKSPACE: u32 = 0xff08;
const KEY_TAB: u32 = 0xff09;
const KEY_RETURN: u32 = 0xff0d;
const KEY_ESCAPE: u32 = 0xff1b;
const KEY_LEFT: u32 = 0xff51;
const KEY_UP: u32 = 0xff52;
const KEY_RIGHT: u32 = 0xff53;
const KEY_DOWN: u32 = 0xff54;
const KEY_PAGE_UP: u32 = 0xff55;
const KEY_PAGE_DOWN: u32 = 0xff56;
const KEY_HANJA: u32 = 0xff34;
const KEY_MODE_SWITCH: u32 = 0xff7e;
const KEY_NUM_LOCK: u32 = 0xff7f;
const KEY_KP_ENTER: u32 = 0xff8d;
const KEY_SHIFT_L: u32 = 0xffe1;
const KEY_HYPER_R: u32 = 0xffee;
pub(crate) const KEY_ALT_R: u32 = 0xffea;
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

/// Make ASCII letter case depend on the Shift key alone.
///
/// With Caps Lock on, xkb reports `R` for the unshifted R key, which would type
/// ㄲ instead of ㄱ. Dubeolsik only wants Shift (ㄲ ㄸ ㅃ ㅆ ㅉ ㅒ ㅖ), so letters
/// are re-cased from `shift`; every other keysym is returned unchanged.
pub fn case_from_shift(keysym: u32, shift: bool) -> u32 {
    match keysym {
        0x41..=0x5a if !shift => keysym + 0x20,
        0x61..=0x7a if shift => keysym - 0x20,
        _ => keysym,
    }
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
        KEY_PAGE_UP => Key::PageUp,
        KEY_PAGE_DOWN => Key::PageDown,
        KEY_HANJA => Key::Hanja,
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
        assert_eq!(translate(0xff34, NONE).key, Key::Hanja);
        assert_eq!(translate(0xff55, NONE).key, Key::PageUp);
        assert_eq!(translate(0xff56, NONE).key, Key::PageDown);
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
    fn caps_lock_does_not_change_letters_without_shift() {
        // Caps Lock on, no Shift: xkb gives 'R' but we want plain 'r'.
        assert_eq!(case_from_shift(0x52, false), 0x72);
        // Caps Lock on plus Shift: xkb gives 'r' but we want 'R'.
        assert_eq!(case_from_shift(0x72, true), 0x52);
    }

    #[test]
    fn case_is_unchanged_when_it_already_matches() {
        assert_eq!(case_from_shift(0x72, false), 0x72);
        assert_eq!(case_from_shift(0x52, true), 0x52);
    }

    #[test]
    fn case_from_shift_leaves_non_letters_alone() {
        for k in [0x31, 0x21, 0x2e, 0x20, 0xff08, 0x5b, 0x7b] {
            assert_eq!(case_from_shift(k, true), k, "{k:#x}");
            assert_eq!(case_from_shift(k, false), k, "{k:#x}");
        }
    }

    #[test]
    fn detects_modifier_keys() {
        for k in [0xffe1, 0xffe2, 0xffe3, 0xffe5, 0xffe9, 0xffeb, 0xfe03] {
            assert!(is_modifier_keysym(k), "{k:#x}");
        }
        assert!(!is_modifier_keysym(0x72));
        assert!(!is_modifier_keysym(0xff08));
    }
}
