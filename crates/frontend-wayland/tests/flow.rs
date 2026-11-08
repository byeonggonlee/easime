//! End-to-end tests of the Wayland frontend against a fake KWin.

mod common;

use common::{Fake, Req, korean_only};

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

// ---- Space, and which releases are forwarded -----------------------------

#[test]
fn space_commits_the_syllable_and_is_forwarded() {
    let mut fake = Fake::start();
    fake.activate();
    fake.tap_all(&[KEY_G, KEY_K]);
    fake.take_requests();
    fake.press(KEY_SPACE);
    let requests = fake.settle();

    let commit = requests
        .iter()
        .position(|r| matches!(r, Req::CommitString { text, .. } if text == "하"))
        .unwrap_or_else(|| panic!("no commit of 하: {requests:#?}"));
    let space = requests
        .iter()
        .position(|r| matches!(r, Req::Key { key, state: 1, .. } if *key == KEY_SPACE))
        .unwrap_or_else(|| panic!("space was not forwarded: {requests:#?}"));
    assert!(commit < space, "commit must precede space: {requests:#?}");
}

#[test]
fn releases_of_consumed_keys_are_dropped_and_of_forwarded_keys_forwarded() {
    let mut fake = Fake::start();
    fake.activate();
    // G and K are consumed by the engine; Space is forwarded.
    fake.press(KEY_G);
    fake.press(KEY_K);
    fake.press(KEY_SPACE);
    fake.release(KEY_G);
    fake.release(KEY_K);
    fake.release(KEY_SPACE);
    let requests = fake.settle();
    assert_eq!(
        forwarded(&requests),
        [(KEY_SPACE, 1), (KEY_SPACE, 0)],
        "{requests:#?}"
    );
}

// ---- Language toggle keys ------------------------------------------------

#[test]
fn keymap_has_the_toggle_keys_where_expected() {
    let fake = Fake::start();
    // These evdev codes are what the tests below rely on.
    assert_eq!(fake.evdev_for_keysym(KEYSYM_HANGUL), KEY_HANGEUL, "Hangul");
    assert_eq!(fake.evdev_for_keysym(KEYSYM_ALT_R), KEY_RIGHTALT, "Alt_R");
}

#[test]
fn hangul_key_toggles_to_english_and_back() {
    let mut fake = Fake::start();
    fake.activate();

    fake.tap_all(&[KEY_HANGEUL]);
    let requests = fake.settle();
    // The toggle key itself is consumed, press and release.
    assert_eq!(forwarded(&requests), [], "{requests:#?}");

    // English: letters are only forwarded, never composed.
    fake.tap_all(&[KEY_G, KEY_K]);
    let requests = fake.settle();
    assert_eq!(
        forwarded(&requests),
        [(KEY_G, 1), (KEY_G, 0), (KEY_K, 1), (KEY_K, 0)],
        "{requests:#?}"
    );
    assert!(commits(&requests).is_empty(), "{requests:#?}");
    assert!(
        !requests
            .iter()
            .any(|r| matches!(r, Req::PreeditString { text, .. } if !text.is_empty())),
        "{requests:#?}"
    );

    // And back to Korean.
    fake.tap_all(&[KEY_HANGEUL]);
    fake.press(KEY_G);
    let requests = fake.settle();
    assert_eq!(forwarded(&requests), [], "{requests:#?}");
    assert_eq!(last_preedit(&requests), Some("ㅎ"), "{requests:#?}");
}

#[test]
fn toggling_commits_the_syllable_in_progress() {
    let mut fake = Fake::start();
    fake.activate();
    fake.tap_all(&[KEY_G, KEY_K]);
    fake.take_requests();
    fake.press(KEY_HANGEUL);
    let requests = fake.settle();
    assert_eq!(commits(&requests), ["하"], "{requests:#?}");
    assert_eq!(last_preedit(&requests), Some(""), "{requests:#?}");
}

#[test]
fn right_alt_press_is_held_back_until_release() {
    let mut fake = Fake::start();
    fake.activate();
    fake.press(KEY_RIGHTALT);
    let requests = fake.settle();
    assert_eq!(requests, [], "the Alt press must be held back");
}

#[test]
fn tapping_right_alt_toggles_the_language() {
    let mut fake = Fake::start();
    fake.activate();

    fake.tap_all(&[KEY_RIGHTALT]);
    let requests = fake.settle();
    assert_eq!(forwarded(&requests), [], "{requests:#?}");

    // English now.
    fake.tap_all(&[KEY_G]);
    let requests = fake.settle();
    assert_eq!(
        forwarded(&requests),
        [(KEY_G, 1), (KEY_G, 0)],
        "{requests:#?}"
    );

    // Tapping again returns to Korean.
    fake.tap_all(&[KEY_RIGHTALT]);
    fake.press(KEY_G);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("ㅎ"), "{requests:#?}");
}

#[test]
fn right_alt_held_with_another_key_is_forwarded_and_does_not_toggle() {
    let mut fake = Fake::start();
    fake.activate();
    let mod1 = fake.mod_mask("Mod1");

    let alt_serial = fake.press(KEY_RIGHTALT);
    fake.modifiers(mod1, 0, 0, 0);
    let key_serial = fake.press(KEY_G);
    let requests = fake.settle();

    // The held-back Alt press comes first, with its own serial, then G.
    assert_eq!(
        forwarded_with_serial(&requests),
        [(alt_serial, KEY_RIGHTALT, 1), (key_serial, KEY_G, 1)],
        "{requests:#?}"
    );

    fake.release(KEY_G);
    fake.modifiers(0, 0, 0, 0);
    fake.release(KEY_RIGHTALT);
    let requests = fake.settle();
    assert_eq!(
        forwarded(&requests),
        [(KEY_G, 0), (KEY_RIGHTALT, 0)],
        "{requests:#?}"
    );

    // Still Korean: no toggle happened.
    fake.press(KEY_G);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("ㅎ"), "{requests:#?}");
}

// ---- Repeated keys -------------------------------------------------------

// Both tests below fail today: wayland-client's generated `KeyState` already
// has `Repeated` (wl_keyboard v10), so a repeated key arrives as
// `WEnum::Value(KeyState::Repeated)`, but `KeyAction::from_wire` only maps
// `WEnum::Unknown(2)` and drops everything else. The unit test in app.rs
// feeds it `Unknown(2)`, which the library never produces for state 2.

#[test]
fn repeated_state_is_forwarded_as_repeated_never_as_release() {
    let mut fake = Fake::start();
    fake.activate();
    fake.tap_all(&[KEY_HANGEUL]);
    fake.press(KEY_G);
    fake.repeat(KEY_G);
    fake.repeat(KEY_G);
    fake.release(KEY_G);
    let requests = fake.settle();
    assert_eq!(
        forwarded(&requests),
        [(KEY_G, 1), (KEY_G, 2), (KEY_G, 2), (KEY_G, 0)],
        "{requests:#?}"
    );
}

#[test]
fn repeated_state_is_forwarded_for_a_passed_key_in_korean_mode_too() {
    let mut fake = Fake::start();
    fake.activate();
    fake.press(KEY_TAB);
    fake.repeat(KEY_TAB);
    fake.release(KEY_TAB);
    let requests = fake.settle();
    assert_eq!(
        forwarded(&requests),
        [(KEY_TAB, 1), (KEY_TAB, 2), (KEY_TAB, 0)],
        "{requests:#?}"
    );
}

// ---- Deactivation --------------------------------------------------------

#[test]
fn deactivate_releases_the_keyboard_and_destroys_the_context() {
    let mut fake = Fake::start();
    fake.activate();
    fake.deactivate();
    let requests = fake.take_requests();
    assert_eq!(
        requests,
        [Req::KeyboardRelease, Req::ContextDestroy],
        "the grab must be released before the context goes away"
    );
}

#[test]
fn deactivate_commits_a_pending_syllable_before_destroying() {
    let mut fake = Fake::start();
    fake.activate();
    fake.tap_all(&[KEY_G, KEY_K]);
    fake.settle();
    let serial = fake.commit_serial;
    fake.deactivate();
    let requests = fake.take_requests();
    assert_eq!(
        requests,
        [
            Req::KeyboardRelease,
            Req::CommitString {
                serial,
                text: "하".into()
            },
            Req::ContextDestroy
        ]
    );
}

#[test]
fn the_input_method_can_be_activated_again_after_deactivation() {
    let mut fake = Fake::start();
    fake.activate();
    fake.press(KEY_G);
    fake.deactivate();
    fake.take_requests();

    fake.activate();
    // A fresh context starts with a clean engine.
    fake.press(KEY_K);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("ㅏ"), "{requests:#?}");
}

// ---- Modifiers and serials -----------------------------------------------

#[test]
fn modifiers_events_are_forwarded_with_the_same_masks() {
    let mut fake = Fake::start();
    fake.activate();
    let shift = fake.mod_mask("Shift");
    let ctrl = fake.mod_mask("Control");
    let caps = fake.mod_mask("Lock");
    let first = fake.modifiers(shift, 0, 0, 0);
    let second = fake.modifiers(shift, ctrl, caps, 0);
    let requests = fake.settle();
    assert_eq!(
        requests,
        [
            Req::Modifiers {
                serial: first,
                depressed: shift,
                latched: 0,
                locked: 0,
                group: 0
            },
            Req::Modifiers {
                serial: second,
                depressed: shift,
                latched: ctrl,
                locked: caps,
                group: 0
            },
        ]
    );
}

#[test]
fn text_requests_carry_the_latest_commit_state_serial() {
    let mut fake = Fake::start();
    fake.activate();
    let first = fake.commit_serial;
    fake.press(KEY_G);
    let requests = fake.settle();
    let serials = text_serials(&requests);
    assert!(!serials.is_empty(), "{requests:#?}");
    assert!(serials.iter().all(|s| *s == first), "{requests:#?}");

    // After a new commit_state, the new serial is used, including for the
    // commit that ends a syllable.
    let second = fake.commit_state();
    assert_ne!(first, second);
    fake.tap_all(&[KEY_K, KEY_S, KEY_R]);
    let requests = fake.settle();
    assert_eq!(commits(&requests), ["한"], "{requests:#?}");
    let serials = text_serials(&requests);
    assert!(!serials.is_empty(), "{requests:#?}");
    assert!(serials.iter().all(|s| *s == second), "{requests:#?}");
}

#[test]
fn key_and_modifiers_requests_carry_the_keyboard_event_serial() {
    let mut fake = Fake::start();
    fake.activate();
    let commit_serial = fake.commit_serial;
    let shift = fake.mod_mask("Shift");
    let mod_serial = fake.modifiers(shift, 0, 0, 0);
    let press_serial = fake.press(KEY_TAB);
    let release_serial = fake.release(KEY_TAB);
    let requests = fake.settle();

    assert_eq!(
        forwarded_with_serial(&requests),
        [(press_serial, KEY_TAB, 1), (release_serial, KEY_TAB, 0)],
        "{requests:#?}"
    );
    assert!(
        matches!(requests.first(), Some(Req::Modifiers { serial, .. }) if *serial == mod_serial),
        "{requests:#?}"
    );
    // The keyboard serials are not the text-input serial.
    assert!(
        [mod_serial, press_serial, release_serial]
            .iter()
            .all(|s| *s != commit_serial)
    );
}

// ---------------------------------------------------------------------------
// Languages and key repeat
// ---------------------------------------------------------------------------

const KEY_ENTER: u32 = 28;
const KEY_A: u32 = 30;
const KEY_N: u32 = 49;

/// Korean and Japanese both enabled, in that order.
fn korean_and_japanese() -> easime_config::Config {
    let mut config = easime_config::Config::default();
    config.languages.enabled = Some(vec![
        ime_core::Language::Korean,
        ime_core::Language::Japanese,
    ]);
    config
}

#[test]
fn toggle_key_cycles_korean_japanese_english() {
    let mut fake = Fake::start_with(korean_and_japanese());
    fake.activate();

    // Korean first. The syllable in progress is committed on every switch.
    fake.tap_all(&[KEY_G, KEY_K]);
    fake.tap_all(&[KEY_HANGEUL]);
    let requests = fake.settle();
    assert_eq!(commits(&requests), ["하"], "{requests:#?}");

    // Japanese: "ka" becomes か.
    fake.tap_all(&[KEY_K, KEY_A]);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("か"), "{requests:#?}");

    // English: the preedit is committed and letters are only forwarded.
    fake.tap_all(&[KEY_HANGEUL]);
    let requests = fake.settle();
    assert_eq!(commits(&requests), ["か"], "{requests:#?}");
    fake.tap_all(&[KEY_G]);
    let requests = fake.settle();
    assert_eq!(
        forwarded(&requests),
        [(KEY_G, 1), (KEY_G, 0)],
        "{requests:#?}"
    );

    // And around to Korean again.
    fake.tap_all(&[KEY_HANGEUL]);
    fake.press(KEY_G);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("ㅎ"), "{requests:#?}");
}

#[test]
fn japanese_enter_commits_the_kana_and_is_consumed() {
    let mut fake = Fake::start_with(korean_and_japanese());
    fake.activate();
    fake.tap_all(&[KEY_HANGEUL]); // Korean -> Japanese

    fake.tap_all(&[KEY_K, KEY_A, KEY_N, KEY_A]);
    fake.tap_all(&[KEY_ENTER]);
    let requests = fake.settle();
    assert_eq!(commits(&requests), ["かな"], "{requests:#?}");
    assert_eq!(forwarded(&requests), [], "Enter must not reach the app");
    assert_eq!(last_preedit(&requests), Some(""), "{requests:#?}");
}

#[test]
fn compositor_repeat_replays_a_consumed_key() {
    // Rate 0: KWin repeats keys itself and sends state 2 to the input method.
    let mut fake = Fake::start();
    fake.activate_with(0, 0);

    fake.press(KEY_G);
    fake.press(KEY_K);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("하"), "{requests:#?}");

    // A repeat of ㅏ cannot extend 하, so 하 is committed and ㅏ starts over.
    fake.repeat(KEY_K);
    let requests = fake.settle();
    assert_eq!(commits(&requests), ["하"], "{requests:#?}");
    assert_eq!(last_preedit(&requests), Some("ㅏ"), "{requests:#?}");
    assert_eq!(forwarded(&requests), [], "consumed keys are not forwarded");
}

#[test]
fn own_timer_repeats_a_held_consumed_key_until_release() {
    // Rate > 0: the input method must repeat consumed keys itself.
    let mut fake = Fake::start();
    fake.activate_with(200, 10);

    fake.press(KEY_G);
    fake.press(KEY_K);
    // No further input: the timer alone produces 하, ㅏ, ㅏ, ...
    fake.wait_for_commits(3);
    fake.release(KEY_K);
    let requests = fake.settle();
    let committed = commits(&requests);
    assert_eq!(&committed[..2], ["하", "ㅏ"], "{requests:#?}");
    assert!(committed[2..].iter().all(|c| *c == "ㅏ"), "{requests:#?}");

    // Released: the timer must stop and the release is not forwarded.
    assert_eq!(forwarded(&requests), [], "{requests:#?}");
    let after_release = commits(&requests).len();
    std::thread::sleep(std::time::Duration::from_millis(80));
    fake.barrier();
    let later = fake.take_requests();
    assert!(
        commits(&later).is_empty(),
        "kept repeating after release (had {after_release}): {later:#?}"
    );
}

#[test]
fn held_backspace_deletes_jamo_then_hands_over_to_the_app() {
    let mut fake = Fake::start();
    fake.activate_with(200, 10);

    fake.tap_all(&[KEY_G, KEY_K]); // 하
    fake.take_requests();
    const KEY_BACKSPACE: u32 = 14;
    fake.press(KEY_BACKSPACE);
    // Own repeat removes ㅏ, then ㅎ, then runs out of composition and the
    // key is handed to the application, which repeats it natively.
    let mut requests = fake.settle();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while forwarded(&requests).is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        fake.barrier();
        requests.extend(fake.take_requests());
    }
    assert_eq!(
        forwarded(&requests),
        [(KEY_BACKSPACE, 1)],
        "Backspace should be forwarded once as a press: {requests:#?}"
    );
    // The eventual release is forwarded because the press was.
    fake.release(KEY_BACKSPACE);
    let requests = fake.settle();
    assert_eq!(forwarded(&requests), [(KEY_BACKSPACE, 0)], "{requests:#?}");
}

// ---------------------------------------------------------------------------
// Live settings reload
// ---------------------------------------------------------------------------

/// A fresh settings file path in a private temp directory.
fn settings_file(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("easime-flow-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("config.toml")
}

#[test]
fn saving_settings_changes_the_toggle_key_without_a_restart() {
    let path = settings_file("toggle");
    let config = korean_only(); // both toggle keys
    config.save_to(&path).unwrap();
    let mut fake = Fake::start_watching(config.clone(), Some(path.clone()));
    fake.activate();

    // Before: the Hangul key toggles to English (and is consumed).
    fake.tap_all(&[KEY_HANGEUL]);
    assert_eq!(forwarded(&fake.settle()), []);
    fake.tap_all(&[KEY_HANGEUL]); // back to Korean

    // The settings app saves "Right Alt only".
    let mut updated = config;
    updated.shortcuts.toggle_key = easime_config::ToggleKey::RightAlt;
    updated.save_to(&path).unwrap();
    fake.barrier();
    fake.take_requests();

    // After: the Hangul key is an ordinary key and reaches the application.
    fake.tap_all(&[KEY_HANGEUL]);
    let requests = fake.settle();
    assert_eq!(
        forwarded(&requests),
        [(KEY_HANGEUL, 1), (KEY_HANGEUL, 0)],
        "{requests:#?}"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn saving_settings_through_a_link_to_another_folder_is_seen_without_a_restart() {
    // Dotfile managers link config.toml to a file in their repository. Saving
    // keeps the link and writes into that folder, which the daemon must watch.
    let path = settings_file("linked");
    let repository = path.parent().unwrap().join("repository");
    std::fs::create_dir_all(&repository).unwrap();
    let real = repository.join("easime.toml");
    let config = korean_only();
    config.save_to(&real).unwrap();
    std::os::unix::fs::symlink(&real, &path).unwrap();
    let mut fake = Fake::start_watching(config.clone(), Some(path.clone()));
    fake.activate();

    fake.tap_all(&[KEY_HANGEUL]);
    assert_eq!(forwarded(&fake.settle()), []);
    fake.tap_all(&[KEY_HANGEUL]);

    let mut updated = config;
    updated.shortcuts.toggle_key = easime_config::ToggleKey::RightAlt;
    updated.save_to(&path).unwrap();
    assert!(
        std::fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink(),
        "saving keeps the link"
    );
    fake.barrier();
    fake.take_requests();

    fake.tap_all(&[KEY_HANGEUL]);
    let requests = fake.settle();
    assert_eq!(
        forwarded(&requests),
        [(KEY_HANGEUL, 1), (KEY_HANGEUL, 0)],
        "{requests:#?}"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn saving_settings_can_enable_japanese_and_keeps_the_current_language() {
    let path = settings_file("languages");
    let config = korean_only();
    config.save_to(&path).unwrap();
    let mut fake = Fake::start_watching(config.clone(), Some(path.clone()));
    fake.activate();

    fake.tap_all(&[KEY_G, KEY_K]); // 하, still being composed
    fake.barrier(); // the client has handled the keys before settings change

    let mut updated = config;
    updated.languages.enabled = Some(vec![
        ime_core::Language::Korean,
        ime_core::Language::Japanese,
    ]);
    updated.save_to(&path).unwrap();
    fake.barrier();
    // Applying settings finishes the text in progress.
    let requests = fake.take_requests();
    assert_eq!(commits(&requests), ["하"], "{requests:#?}");

    // Still Korean, and the cycle now continues to Japanese.
    fake.press(KEY_G);
    assert_eq!(last_preedit(&fake.settle()), Some("ㅎ"));
    fake.tap_all(&[KEY_HANGEUL]);
    fake.tap_all(&[KEY_K, KEY_A]);
    assert_eq!(last_preedit(&fake.settle()), Some("か"));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn an_unreadable_settings_file_is_ignored() {
    let path = settings_file("broken");
    let config = korean_only();
    config.save_to(&path).unwrap();
    let mut fake = Fake::start_watching(config, Some(path.clone()));
    fake.activate();

    fake.barrier();
    std::fs::write(&path, "this is = = not toml").unwrap();
    fake.barrier();
    fake.take_requests();

    // Typing keeps working with the previous settings.
    fake.tap_all(&[KEY_G, KEY_K]);
    assert_eq!(last_preedit(&fake.settle()), Some("하"));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
