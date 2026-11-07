//! End-to-end tests of the Wayland frontend against a fake KWin.

mod common;

use common::{Fake, Req};

// Linux evdev key codes (input-event-codes.h).
const KEY_R: u32 = 19;
const KEY_S: u32 = 31;
const KEY_F: u32 = 33;
const KEY_G: u32 = 34;
const KEY_K: u32 = 37;
const KEY_M: u32 = 50;

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
    assert!(
        !requests.iter().any(|r| matches!(r, Req::Key { .. })),
        "{requests:#?}"
    );
}
