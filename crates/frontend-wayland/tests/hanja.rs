//! The whole Hanja interaction, over the real Wayland client connection.

mod common;

use common::{Fake, Globals, Req};
use easime_config::Config;
use std::time::Duration;

const HANJA: u32 = 0xff34;
const ENTER: u32 = 28;
const ESCAPE: u32 = 1;
const BACKSPACE: u32 = 14;
const DOWN: u32 = 108;

fn type_han(fake: &mut Fake) {
    fake.tap_all(&[34, 37, 31]);
    let _ = fake.settle_panel();
    let _ = fake.take_requests();
}

#[test]
fn hanja_candidates_commit_once_without_forwarding_enter() {
    let mut fake = Fake::start();
    fake.activate();
    type_han(&mut fake);
    let hanja = fake.evdev_for_keysym(HANJA);
    fake.tap_all(&[hanja]);
    let shot = fake.wait_for_indicator_shown();
    assert!(shot.width > 48 && shot.height > 48);
    fake.take_requests();
    fake.tap_all(&[DOWN, ENTER]);
    let requests = fake.settle();
    let commits: Vec<_> = requests
        .iter()
        .filter_map(|r| match r {
            Req::CommitString { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(commits, ["漢"]);
    assert!(
        !requests
            .iter()
            .any(|r| matches!(r, Req::Key { key: ENTER, .. }))
    );
    fake.wait_for_indicator_hidden();
}

#[test]
fn escape_returns_to_editable_hangul() {
    let mut fake = Fake::start();
    fake.activate();
    type_han(&mut fake);
    let hanja = fake.evdev_for_keysym(HANJA);
    fake.tap_all(&[hanja, ESCAPE, BACKSPACE]);
    let requests = fake.settle();
    let text = requests.iter().rev().find_map(|r| match r {
        Req::PreeditString { text, .. } => Some(text.as_str()),
        _ => None,
    });
    assert_eq!(text, Some("하"));
    assert!(
        !requests
            .iter()
            .any(|r| matches!(r, Req::CommitString { .. }))
    );
}

#[test]
fn an_unavailable_panel_never_enables_invisible_conversion() {
    let mut fake = Fake::start_with_globals(
        Config::default(),
        Globals {
            input_panel: false,
            ..Globals::default()
        },
    );
    fake.activate();
    type_han(&mut fake);
    let hanja = fake.evdev_for_keysym(HANJA);
    fake.tap_all(&[hanja, ENTER]);
    let requests = fake.settle();
    let commits: Vec<_> = requests
        .iter()
        .filter_map(|r| match r {
            Req::CommitString { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(commits, ["한"]);
    assert!(requests.iter().any(|r| matches!(
        r,
        Req::Key {
            key: ENTER,
            state: 1,
            ..
        }
    )));
}

#[test]
fn indicator_timeout_does_not_hide_candidates() {
    let mut config = Config::default();
    config.appearance.indicator_ms = 120;
    let mut fake = Fake::start_with(config);
    fake.activate();
    // Arm the indicator timer explicitly, then open candidates in Korean.
    for _ in 0..2 {
        fake.tap_all(&[122]);
        fake.wait_for_indicator_shown();
    }
    type_han(&mut fake);
    let hanja = fake.evdev_for_keysym(HANJA);
    fake.tap_all(&[hanja]);
    let shot = fake.wait_for_indicator_shown();
    assert!(shot.width > 48);
    std::thread::sleep(Duration::from_millis(220));
    fake.barrier();
    assert!(fake.indicator_mapped());
    fake.tap_all(&[ENTER]);
    fake.wait_for_indicator_hidden();
}

#[test]
fn candidates_work_with_the_indicator_disabled() {
    let mut config = Config::default();
    config.appearance.indicator_box = false;
    let mut fake = Fake::start_with(config);
    fake.activate();
    type_han(&mut fake);
    let hanja = fake.evdev_for_keysym(HANJA);
    fake.tap_all(&[hanja]);
    assert!(fake.wait_for_indicator_shown().width > 48);
}

#[test]
fn japanese_does_not_show_unimplemented_hanja_page_controls() {
    let mut config = Config::default();
    config.appearance.indicator_box = false;
    config.languages.enabled = vec![ime_core::Language::Japanese];
    let mut fake = Fake::start_with(config);
    fake.activate();
    fake.tap_all(&[37, 30, 57]); // ka + Space
    let _ = fake.settle_panel();
    assert!(!fake.indicator_mapped());
}
