//! End-to-end tests of the Wayland frontend against a fake KWin.

mod common;

use common::{Fake, Req};

// Linux evdev key codes (input-event-codes.h).
const KEY_TAB: u32 = 15;
const KEY_R: u32 = 19;
const KEY_S: u32 = 31;
const KEY_F: u32 = 33;
const KEY_G: u32 = 34;
const KEY_K: u32 = 37;
const KEY_M: u32 = 50;
const KEY_SPACE: u32 = 57;
const KEY_RIGHTALT: u32 = 100;
const KEY_HANGEUL: u32 = 122;

// XKB keysyms of the toggle keys.
const KEYSYM_HANGUL: u32 = 0xff31;
const KEYSYM_ALT_R: u32 = 0xffea;

/// Every `commit_string` text, in order.
fn commits(requests: &[Req]) -> Vec<&str> {
    requests
        .iter()
        .filter_map(|r| match r {
            Req::CommitString { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// The text of the last `preedit_string`.
fn last_preedit(requests: &[Req]) -> Option<&str> {
    requests.iter().rev().find_map(|r| match r {
        Req::PreeditString { text, .. } => Some(text.as_str()),
        _ => None,
    })
}

/// The `(key, state)` pairs of every forwarded `key` request.
fn forwarded(requests: &[Req]) -> Vec<(u32, u32)> {
    requests
        .iter()
        .filter_map(|r| match r {
            Req::Key { key, state, .. } => Some((*key, *state)),
            _ => None,
        })
        .collect()
}

/// The `(serial, key, state)` triples of every forwarded `key` request.
fn forwarded_with_serial(requests: &[Req]) -> Vec<(u32, u32, u32)> {
    requests
        .iter()
        .filter_map(|r| match r {
            Req::Key {
                serial, key, state, ..
            } => Some((*serial, *key, *state)),
            _ => None,
        })
        .collect()
}

/// The serial of every `commit_string` and `preedit_string`.
fn text_serials(requests: &[Req]) -> Vec<u32> {
    requests
        .iter()
        .filter_map(|r| match r {
            Req::PreeditString { serial, .. } | Req::CommitString { serial, .. } => Some(*serial),
            _ => None,
        })
        .collect()
}

// ---- Activation and typing -----------------------------------------------

#[test]
fn client_binds_the_input_method_global() {
    let _fake = Fake::start();
}

#[test]
fn activation_grabs_the_keyboard_and_loads_the_keymap() {
    let mut fake = Fake::start();
    fake.activate();
    // The keymap arrived and was compiled: typing produces Hangul rather than
    // being forwarded untranslated.
    fake.press(KEY_G);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("ㅎ"), "{requests:#?}");
}

#[test]
fn typing_gksrmf_commits_han_and_leaves_geul_as_preedit() {
    let mut fake = Fake::start();
    fake.activate();
    fake.tap_all(&[KEY_G, KEY_K, KEY_S, KEY_R, KEY_M, KEY_F]);
    let requests = fake.settle();
    assert_eq!(commits(&requests), ["한"], "{requests:#?}");
    assert_eq!(last_preedit(&requests), Some("글"), "{requests:#?}");
    // Nothing was forwarded to the application as a raw key.
    assert_eq!(forwarded(&requests), [], "{requests:#?}");
}

#[test]
fn preedit_is_styled_and_the_cursor_placed_before_each_preedit_string() {
    let mut fake = Fake::start();
    fake.activate();
    fake.press(KEY_G);
    let requests = fake.settle();
    // "ㅎ" is three bytes in UTF-8; style 5 is underline.
    assert_eq!(
        requests,
        [
            Req::PreeditStyling {
                index: 0,
                length: 3,
                style: 5
            },
            Req::PreeditCursor { index: 3 },
            Req::PreeditString {
                serial: fake.commit_serial,
                text: "ㅎ".into(),
                commit: "ㅎ".into()
            },
        ]
    );
}

// ---- Shift and Caps Lock -------------------------------------------------

#[test]
fn plain_r_types_a_plain_consonant() {
    let mut fake = Fake::start();
    fake.activate();
    fake.press(KEY_R);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("ㄱ"), "{requests:#?}");
}

#[test]
fn shift_r_types_a_tense_consonant() {
    let mut fake = Fake::start();
    fake.activate();
    let shift = fake.mod_mask("Shift");
    fake.modifiers(shift, 0, 0, 0);
    fake.press(KEY_R);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("ㄲ"), "{requests:#?}");
}

#[test]
fn caps_lock_does_not_double_consonants() {
    let mut fake = Fake::start();
    fake.activate();
    let caps = fake.mod_mask("Lock");
    fake.modifiers(0, 0, caps, 0);
    fake.press(KEY_R);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("ㄱ"), "{requests:#?}");

    // Whole syllables are unaffected by the lock, too.
    fake.release(KEY_R);
    fake.tap_all(&[KEY_G, KEY_K, KEY_S]);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("한"), "{requests:#?}");
}

#[test]
fn shift_still_works_while_caps_lock_is_on() {
    let mut fake = Fake::start();
    fake.activate();
    let shift = fake.mod_mask("Shift");
    let caps = fake.mod_mask("Lock");
    fake.modifiers(shift, 0, caps, 0);
    fake.press(KEY_R);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("ㄲ"), "{requests:#?}");
}

