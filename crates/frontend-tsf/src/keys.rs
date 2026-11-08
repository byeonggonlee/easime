//! Windows key presses as `ime-core` key events.
//!
//! A TSF key event sink gets a virtual-key code (`wParam`) and the `lParam`
//! word of the equivalent `WM_KEYDOWN`. This module reads those and the
//! keyboard state array (`GetKeyboardState`), all plain numbers, so it is
//! tested on every OS.
//!
//! Letters, digits and punctuation are taken from the **scan code**, the
//! physical key, as a US QWERTY keyboard would label it. That is what the
//! Hangul Dubeolsik and romaji engines expect (`Key::Char` is "the character
//! on a US layout"), and it means Korean typing works on any keyboard layout,
//! like the Microsoft Korean input method. The Wayland frontend uses the layout's
//! keysym instead (a known limitation there, `docs/status.md`).

use ime_core::{Key, KeyEvent, Modifiers};
use ime_session::KeyRole;

// Virtual-key codes (winuser.h).
pub const VK_BACK: u16 = 0x08;
pub const VK_TAB: u16 = 0x09;
pub const VK_RETURN: u16 = 0x0D;
pub const VK_SHIFT: u16 = 0x10;
pub const VK_CONTROL: u16 = 0x11;
/// The Alt key. A TSF key sink gets this one for both Alt keys; the right one
/// has the extended-key flag ([`KeyInfo::extended`]).
pub const VK_MENU: u16 = 0x12;
pub const VK_CAPITAL: u16 = 0x14;
/// The Hangul (Korean/English) key, `VK_HANGUL` = `VK_KANA`.
pub const VK_HANGUL: u16 = 0x15;
/// The Hanja key, `VK_HANJA` = `VK_KANJI`.
pub const VK_HANJA: u16 = 0x19;
pub const VK_ESCAPE: u16 = 0x1B;
pub const VK_SPACE: u16 = 0x20;
pub const VK_PRIOR: u16 = 0x21;
pub const VK_NEXT: u16 = 0x22;
pub const VK_LEFT: u16 = 0x25;
pub const VK_UP: u16 = 0x26;
pub const VK_RIGHT: u16 = 0x27;
pub const VK_DOWN: u16 = 0x28;
pub const VK_LWIN: u16 = 0x5B;
pub const VK_RWIN: u16 = 0x5C;
pub const VK_NUMPAD0: u16 = 0x60;
pub const VK_NUMPAD9: u16 = 0x69;
pub const VK_MULTIPLY: u16 = 0x6A;
pub const VK_ADD: u16 = 0x6B;
pub const VK_SUBTRACT: u16 = 0x6D;
pub const VK_DECIMAL: u16 = 0x6E;
pub const VK_DIVIDE: u16 = 0x6F;
pub const VK_LSHIFT: u16 = 0xA0;
pub const VK_RSHIFT: u16 = 0xA1;
pub const VK_LCONTROL: u16 = 0xA2;
pub const VK_RCONTROL: u16 = 0xA3;
pub const VK_LMENU: u16 = 0xA4;
pub const VK_RMENU: u16 = 0xA5;
/// Text typed by a program rather than a key (`SendInput` with
/// `KEYEVENTF_UNICODE`: remote desktops, on-screen tools, password managers).
/// Its scan-code field holds part of the character, not a key position.
pub const VK_PACKET: u16 = 0xE7;

/// What Windows tells a key event sink about one key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyInfo {
    /// The virtual-key code (`wParam`).
    pub vk: u16,
    /// The scan code: which physical key (`lParam` bits 16–23).
    pub scan: u8,
    /// The extended-key flag (`lParam` bit 24): the right-hand Alt and Ctrl,
    /// the arrow keys of the navigation block, the numpad's `/` and Enter.
    pub extended: bool,
    /// The key was already down (`lParam` bit 30): this press is a repeat.
    pub repeat: bool,
}

impl KeyInfo {
    /// From the virtual-key code and the `lParam` word of a key message.
    pub fn from_message(vk: u16, lparam: isize) -> Self {
        let lparam = lparam as u32;
        Self {
            vk,
            scan: ((lparam >> 16) & 0xFF) as u8,
            extended: lparam & (1 << 24) != 0,
            repeat: lparam & (1 << 30) != 0,
        }
    }
}

/// What a key is to the language switch (see `ime_session::toggle`). The
/// Hangul key, and the right Alt key. On Korean 101-key keyboards the right Alt
/// *is* the Hangul key and arrives as `VK_HANGUL`.
///
/// Known limit: on layouts with AltGr, the right Alt key makes Windows send a
/// left Ctrl first. The caller must report `KeyRole::Other` there; see
/// `docs/platforms.md`.
pub fn toggle_role(key: KeyInfo) -> KeyRole {
    match key.vk {
        VK_HANGUL => KeyRole::HangulKey,
        VK_RMENU => KeyRole::RightAlt,
        VK_MENU if key.extended => KeyRole::RightAlt,
        _ => KeyRole::Other,
    }
}

/// True for the keys that only change what other keys mean: they are never
/// text and never reach an engine.
pub fn is_modifier(vk: u16) -> bool {
    matches!(
        vk,
        VK_SHIFT
            | VK_CONTROL
            | VK_MENU
            | VK_CAPITAL
            | VK_LWIN
            | VK_RWIN
            | VK_LSHIFT
            | VK_RSHIFT
            | VK_LCONTROL
            | VK_RCONTROL
            | VK_LMENU
            | VK_RMENU
    )
}

/// The keys with a name of their own (not letters or symbols).
pub fn named_key(vk: u16) -> Option<Key> {
    Some(match vk {
        VK_BACK => Key::Backspace,
        VK_TAB => Key::Tab,
        VK_RETURN => Key::Enter,
        VK_ESCAPE => Key::Escape,
        VK_SPACE => Key::Space,
        VK_PRIOR => Key::PageUp,
        VK_NEXT => Key::PageDown,
        VK_LEFT => Key::Left,
        VK_UP => Key::Up,
        VK_RIGHT => Key::Right,
        VK_DOWN => Key::Down,
        VK_HANJA => Key::Hanja,
        // Never a key position, whatever its scan-code field says: composing it
        // would turn a pasted "!" into a Hangul jamo.
        VK_PACKET => Key::Other,
        _ => return None,
    })
}

/// The character a physical key makes on a US QWERTY keyboard, from its
/// (non-extended) scan code. `None` for keys that make no character.
pub fn us_char(scan: u8, shift: bool) -> Option<char> {
    // Scan code set 1 (what Windows reports), in the order of the rows.
    const UNSHIFTED: &[(u8, char)] = &[
        (0x29, '`'),
        (0x02, '1'),
        (0x03, '2'),
        (0x04, '3'),
        (0x05, '4'),
        (0x06, '5'),
        (0x07, '6'),
        (0x08, '7'),
        (0x09, '8'),
        (0x0A, '9'),
        (0x0B, '0'),
        (0x0C, '-'),
        (0x0D, '='),
        (0x10, 'q'),
        (0x11, 'w'),
        (0x12, 'e'),
        (0x13, 'r'),
        (0x14, 't'),
        (0x15, 'y'),
        (0x16, 'u'),
        (0x17, 'i'),
        (0x18, 'o'),
        (0x19, 'p'),
        (0x1A, '['),
        (0x1B, ']'),
        (0x2B, '\\'),
        (0x1E, 'a'),
        (0x1F, 's'),
        (0x20, 'd'),
        (0x21, 'f'),
        (0x22, 'g'),
        (0x23, 'h'),
        (0x24, 'j'),
        (0x25, 'k'),
        (0x26, 'l'),
        (0x27, ';'),
        (0x28, '\''),
        (0x2C, 'z'),
        (0x2D, 'x'),
        (0x2E, 'c'),
        (0x2F, 'v'),
        (0x30, 'b'),
        (0x31, 'n'),
        (0x32, 'm'),
        (0x33, ','),
        (0x34, '.'),
    ];
    let unshifted = UNSHIFTED
        .iter()
        .find(|(code, _)| *code == scan)
        .map(|&(_, c)| c)
        .or((scan == 0x35).then_some('/'))?;
    if !shift {
        return Some(unshifted);
    }
    Some(match unshifted {
        'a'..='z' => unshifted.to_ascii_uppercase(),
        '`' => '~',
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        other => other,
    })
}

/// The character of a numpad key (with Num Lock on Windows reports these
/// virtual keys), if it is one.
pub fn numpad_char(vk: u16) -> Option<char> {
    match vk {
        VK_NUMPAD0..=VK_NUMPAD9 => char::from_digit(u32::from(vk - VK_NUMPAD0), 10),
        VK_MULTIPLY => Some('*'),
        VK_ADD => Some('+'),
        VK_SUBTRACT => Some('-'),
        VK_DECIMAL => Some('.'),
        VK_DIVIDE => Some('/'),
        _ => None,
    }
}

/// Windows' keyboard state array (`GetKeyboardState`): 256 bytes, the high bit
/// of a key's byte is set while it is down.
pub type KeyboardState = [u8; 256];

fn down(state: &KeyboardState, vk: u16) -> bool {
    state[usize::from(vk)] & 0x80 != 0
}

/// Whether Shift is held. Caps Lock does not count: Dubeolsik and romaji look
/// at the keys themselves.
pub fn shift_held(state: &KeyboardState) -> bool {
    down(state, VK_SHIFT)
}

/// The non-Shift modifiers held.
pub fn modifiers(state: &KeyboardState) -> Modifiers {
    Modifiers {
        ctrl: down(state, VK_CONTROL),
        alt: down(state, VK_MENU),
        meta: down(state, VK_LWIN) || down(state, VK_RWIN),
    }
}

/// The event an engine gets for a key press, or `None` for the keys that only
/// change what other keys mean (Shift, Ctrl, Alt, Caps Lock, the Windows key).
/// The US scan code of a letter or digit key, from its virtual key ('A'..'Z',
/// '0'..'9' are their own virtual-key codes).
fn scan_of_vk(vk: u16) -> Option<u8> {
    let wanted = match vk {
        0x30..=0x39 | 0x41..=0x5A => char::from(vk as u8).to_ascii_lowercase(),
        _ => return None,
    };
    (1..=0x58u8).find(|&scan| us_char(scan, false) == Some(wanted))
}

pub fn key_event(key: KeyInfo, state: &KeyboardState) -> Option<KeyEvent> {
    if is_modifier(key.vk) {
        return None;
    }
    let found = named_key(key.vk)
        .or_else(|| numpad_char(key.vk).map(Key::Char))
        .or_else(|| {
            // An extended scan code is a different key (the numpad's `/`
            // shares 0x35 with the US `/`), and those have a virtual key of
            // their own, handled above.
            (!key.extended)
                .then(|| us_char(key.scan, shift_held(state)).map(Key::Char))
                .flatten()
        })
        .or_else(|| {
            // Programs that send keys sometimes leave the scan code out. A
            // letter or a digit is then known by its virtual key alone.
            (key.scan == 0)
                .then(|| scan_of_vk(key.vk))
                .flatten()
                .and_then(|scan| us_char(scan, shift_held(state)).map(Key::Char))
        })
        .unwrap_or(Key::Other);
    Some(KeyEvent {
        key: found,
        modifiers: modifiers(state),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_KEYS: KeyboardState = [0; 256];

    fn held(keys: &[u16]) -> KeyboardState {
        let mut state = NO_KEYS;
        for &vk in keys {
            state[usize::from(vk)] = 0x80;
        }
        state
    }

    fn press(vk: u16, scan: u8) -> KeyInfo {
        KeyInfo {
            vk,
            scan,
            extended: false,
            repeat: false,
        }
    }

    #[test]
    fn the_lparam_of_a_key_message_is_taken_apart() {
        // 'A': one repeat, scan code 0x1E.
        let a = KeyInfo::from_message(0x41, (0x1E << 16) | 1);
        assert_eq!(
            (a.vk, a.scan, a.extended, a.repeat),
            (0x41, 0x1E, false, false)
        );
        // Right Alt: scan code 0x38 with the extended flag, held (previous state set).
        let alt = KeyInfo::from_message(VK_MENU, (1 << 30) | (1 << 24) | (0x38 << 16) | 1);
        assert_eq!((alt.scan, alt.extended, alt.repeat), (0x38, true, true));
        // A negative isize (the top bit set) is read as the same bits.
        let release = KeyInfo::from_message(0x41, (0x1E << 16) as isize | isize::MIN | (1 << 30));
        assert_eq!(release.scan, 0x1E);
    }

    #[test]
    fn only_the_hangul_key_and_the_right_alt_switch_language() {
        assert_eq!(toggle_role(press(VK_HANGUL, 0x72)), KeyRole::HangulKey);
        assert_eq!(toggle_role(press(VK_RMENU, 0x38)), KeyRole::RightAlt);
        let right_alt = KeyInfo {
            extended: true,
            ..press(VK_MENU, 0x38)
        };
        assert_eq!(toggle_role(right_alt), KeyRole::RightAlt);
        // The left Alt is the same virtual key without the extended flag.
        assert_eq!(toggle_role(press(VK_MENU, 0x38)), KeyRole::Other);
        assert_eq!(toggle_role(press(VK_LMENU, 0x38)), KeyRole::Other);
        assert_eq!(toggle_role(press(0x41, 0x1E)), KeyRole::Other);
        assert_eq!(toggle_role(press(VK_HANJA, 0x71)), KeyRole::Other);
    }

    #[test]
    fn every_letter_key_is_its_us_letter() {
        let rows = [
            (0x10..=0x19u8, "qwertyuiop"),
            (0x1E..=0x26u8, "asdfghjkl"),
            (0x2C..=0x32u8, "zxcvbnm"),
        ];
        for (scans, letters) in rows {
            for (scan, letter) in scans.zip(letters.chars()) {
                assert_eq!(us_char(scan, false), Some(letter), "scan {scan:#x}");
                assert_eq!(us_char(scan, true), Some(letter.to_ascii_uppercase()));
            }
        }
    }

    #[test]
    fn digits_and_punctuation_follow_the_us_labels() {
        let digits = "1234567890";
        for (i, digit) in digits.chars().enumerate() {
            assert_eq!(us_char(0x02 + i as u8, false), Some(digit));
        }
        let shifted: String = (0..10).filter_map(|i| us_char(0x02 + i, true)).collect();
        assert_eq!(shifted, "!@#$%^&*()");
        let pairs = [
            (0x29, '`', '~'),
            (0x0C, '-', '_'),
            (0x0D, '=', '+'),
            (0x1A, '[', '{'),
            (0x1B, ']', '}'),
            (0x2B, '\\', '|'),
            (0x27, ';', ':'),
            (0x28, '\'', '"'),
            (0x33, ',', '<'),
            (0x34, '.', '>'),
            (0x35, '/', '?'),
        ];
        for (scan, plain, shifted) in pairs {
            assert_eq!(us_char(scan, false), Some(plain), "scan {scan:#x}");
            assert_eq!(us_char(scan, true), Some(shifted), "scan {scan:#x}");
        }
    }

    #[test]
    fn keys_that_make_no_character_have_none() {
        for scan in [
            0x00, 0x01, 0x0E, 0x0F, 0x1C, 0x1D, 0x2A, 0x36, 0x38, 0x39, 0x3A, 0x3B, 0x53,
        ] {
            assert_eq!(us_char(scan, false), None, "scan {scan:#x}");
        }
    }

    #[test]
    fn named_keys_and_the_hanja_key() {
        assert_eq!(named_key(VK_BACK), Some(Key::Backspace));
        assert_eq!(named_key(VK_RETURN), Some(Key::Enter));
        assert_eq!(named_key(VK_ESCAPE), Some(Key::Escape));
        assert_eq!(named_key(VK_SPACE), Some(Key::Space));
        assert_eq!(named_key(VK_TAB), Some(Key::Tab));
        assert_eq!(named_key(VK_PRIOR), Some(Key::PageUp));
        assert_eq!(named_key(VK_NEXT), Some(Key::PageDown));
        assert_eq!(named_key(VK_LEFT), Some(Key::Left));
        assert_eq!(named_key(VK_DOWN), Some(Key::Down));
        assert_eq!(named_key(VK_HANJA), Some(Key::Hanja));
        assert_eq!(named_key(0x41), None);
    }

    #[test]
    fn a_letter_comes_from_its_scan_code_whatever_the_layout() {
        // A German keyboard's Z key has the virtual key 'Y' but the scan code of
        // the US Y key; the engines want the US letter of the physical key.
        let event = key_event(press(0x59, 0x2C), &NO_KEYS).unwrap();
        assert_eq!(event.key, Key::Char('z'));
        let event = key_event(press(0x41, 0x1E), &held(&[VK_SHIFT])).unwrap();
        assert_eq!(event.key, Key::Char('A'));
    }

    #[test]
    fn caps_lock_does_not_change_letters() {
        let mut state = NO_KEYS;
        state[usize::from(VK_CAPITAL)] = 0x01; // toggled on, not held down
        let event = key_event(press(0x52, 0x13), &state).unwrap();
        assert_eq!(event.key, Key::Char('r'));
    }

    #[test]
    fn the_numpad_makes_digits_and_symbols() {
        let event = key_event(press(VK_NUMPAD0 + 7, 0x48), &NO_KEYS).unwrap();
        assert_eq!(event.key, Key::Char('7'));
        let event = key_event(press(VK_DIVIDE, 0x35), &NO_KEYS).unwrap();
        assert_eq!(event.key, Key::Char('/'));
        let extended_slash = KeyInfo {
            extended: true,
            ..press(VK_DIVIDE, 0x35)
        };
        assert_eq!(
            key_event(extended_slash, &NO_KEYS).unwrap().key,
            Key::Char('/')
        );
    }

    #[test]
    fn an_extended_scan_code_is_not_the_us_key_of_the_same_number() {
        // Some extended key that shares a number with a letter key makes no
        // letter: its virtual key is what tells it apart, and an unknown one is Other.
        let odd = KeyInfo {
            extended: true,
            ..press(0xE7, 0x1E)
        };
        assert_eq!(key_event(odd, &NO_KEYS).unwrap().key, Key::Other);
    }

    #[test]
    fn text_sent_by_a_program_is_never_read_as_a_key_position() {
        // "!" sent as Unicode text: the scan-code field holds 0x21, which is the
        // position of F on a US keyboard. It must not become a Hangul jamo.
        for scan in [0x21, 0x1E, 0x00] {
            assert_eq!(
                key_event(press(VK_PACKET, scan), &NO_KEYS).unwrap().key,
                Key::Other
            );
        }
    }

    #[test]
    fn a_letter_or_digit_sent_without_a_scan_code_is_known_by_its_virtual_key() {
        assert_eq!(
            key_event(press(0x47, 0), &NO_KEYS).unwrap().key,
            Key::Char('g')
        );
        assert_eq!(
            key_event(press(0x47, 0), &held(&[VK_SHIFT])).unwrap().key,
            Key::Char('G')
        );
        assert_eq!(
            key_event(press(0x31, 0), &NO_KEYS).unwrap().key,
            Key::Char('1')
        );
        assert_eq!(
            key_event(press(0x31, 0), &held(&[VK_SHIFT])).unwrap().key,
            Key::Char('!')
        );
        // Other keys without a scan code stay unknown.
        assert_eq!(key_event(press(0x70, 0), &NO_KEYS).unwrap().key, Key::Other);
    }

    #[test]
    fn modifier_keys_alone_are_not_events() {
        for vk in [
            VK_SHIFT, VK_CONTROL, VK_MENU, VK_CAPITAL, VK_LWIN, VK_RWIN, VK_RMENU, VK_LSHIFT,
        ] {
            assert_eq!(key_event(press(vk, 0), &NO_KEYS), None, "vk {vk:#x}");
        }
    }

    #[test]
    fn held_modifiers_are_carried_with_the_key() {
        let event = key_event(press(0x43, 0x2E), &held(&[VK_CONTROL])).unwrap();
        assert_eq!(event.key, Key::Char('c'));
        assert!(event.modifiers.ctrl && !event.modifiers.alt && !event.modifiers.meta);
        let event = key_event(press(0x43, 0x2E), &held(&[VK_MENU, VK_LWIN])).unwrap();
        assert!(!event.modifiers.ctrl && event.modifiers.alt && event.modifiers.meta);
        // Shift is not a modifier in this sense: it only changes the character.
        let event = key_event(press(0x43, 0x2E), &held(&[VK_SHIFT])).unwrap();
        assert_eq!(event.modifiers, Modifiers::NONE);
    }

    #[test]
    fn the_unknown_keys_are_other() {
        // F1.
        assert_eq!(
            key_event(press(0x70, 0x3B), &NO_KEYS).unwrap().key,
            Key::Other
        );
    }
}
