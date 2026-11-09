//! Requests from another thread (the tray icon): switching language by name
//! and to the next one. The test thread plays the requesting thread; the
//! input method runs on its own, as in the daemon.

mod common;

use common::{Fake, Req};
use easime_config::Config;
use ime_core::Language;
use std::time::{Duration, Instant};

const KEY_G: u32 = 34;
const KEY_K: u32 = 37;
const KEY_A: u32 = 30;

fn commits(requests: &[Req]) -> Vec<&str> {
    requests
        .iter()
        .filter_map(|r| match r {
            Req::CommitString { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn last_preedit(requests: &[Req]) -> Option<&str> {
    requests.iter().rev().find_map(|r| match r {
        Req::PreeditString { text, .. } => Some(text.as_str()),
        _ => None,
    })
}

/// Requests from another thread arrive when they arrive: keep letting the
/// input method catch up until `done` says what was waited for has happened.
fn wait_for(fake: &mut Fake, done: impl Fn(&[Req]) -> bool) -> Vec<Req> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen = Vec::new();
    while Instant::now() < deadline {
        seen.extend(fake.settle());
        if done(&seen) {
            return seen;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the input method did not do what was asked: {seen:#?}");
}

#[test]
fn another_thread_can_switch_the_language() {
    let mut config = Config::default();
    config.languages.enabled = Some(vec![Language::Korean, Language::Japanese]);
    config.languages.switch = ime_core::SwitchMode::Cycle;
    let mut fake = Fake::start_with(config);
    fake.activate();

    // Korean, a syllable in progress.
    fake.tap_all(&[KEY_G, KEY_K]);
    assert_eq!(last_preedit(&fake.settle()), Some("하"));

    // "Japanese, please": the syllable is committed first, then kana come out.
    frontend_wayland::select_language(Language::Japanese);
    let requests = wait_for(&mut fake, |r| !commits(r).is_empty());
    assert_eq!(commits(&requests), ["하"], "{requests:#?}");
    fake.tap_all(&[KEY_K, KEY_A]);
    assert_eq!(last_preedit(&fake.settle()), Some("か"));

    // The language it is in already: nothing happens, the kana stay.
    frontend_wayland::select_language(Language::Japanese);
    std::thread::sleep(Duration::from_millis(100));
    let requests = fake.settle();
    assert!(commits(&requests).is_empty(), "{requests:#?}");

    // "Next", as the switch key: English, so the kana are committed.
    frontend_wayland::next_language();
    let requests = wait_for(&mut fake, |r| !commits(r).is_empty());
    assert_eq!(commits(&requests), ["か"], "{requests:#?}");

    // A language that is not in the cycle is ignored.
    frontend_wayland::select_language(Language::ChineseTraditional);
    std::thread::sleep(Duration::from_millis(100));
    let requests = fake.settle();
    assert!(commits(&requests).is_empty(), "{requests:#?}");
}
