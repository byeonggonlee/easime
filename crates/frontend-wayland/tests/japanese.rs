//! Japanese conversion with the dictionary, over the real Wayland client
//! connection: the candidate window, the preedit and what reaches the
//! application.

mod common;

use common::{Fake, Globals, Req};
use easime_config::Config;
use ime_core::Language;

const ESCAPE: u32 = 1;
const KEY_2: u32 = 3;
const BACKSPACE: u32 = 14;
const ENTER: u32 = 28;
const SPACE: u32 = 57;
const UP: u32 = 103;
const PAGE_UP: u32 = 104;
const DOWN: u32 = 108;
const PAGE_DOWN: u32 = 109;
const TOGGLE: u32 = 122;

const H: u32 = 35;
const A: u32 = 30;
const S: u32 = 31;
const I: u32 = 23;
const K: u32 = 37;
const N: u32 = 49;
const U: u32 = 22;
const R: u32 = 19;
const M: u32 = 50;

const HASHI: [u32; 5] = [H, A, S, H, I];
const KANA: [u32; 4] = [K, A, N, A];
const KURUMA: [u32; 6] = [K, U, R, U, M, A];

/// "はし" has three readings' worth of words; "かな" has twenty candidates, so
/// 22 with the two kana ones: pages of 9, 9 and 4.
fn dictionary() -> String {
    let words: Vec<String> = (1..=20).map(|n| format!("案{n}")).collect();
    format!("はし\t橋,箸,端\nかな\t{}\n", words.join(","))
}

/// Japanese is the only language, so typing starts in Japanese.
fn japanese() -> Config {
    let mut config = Config::default();
    config.languages.enabled = vec![Language::Japanese];
    config
}

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

/// True if the key press itself was handed on to the application.
fn forwarded(requests: &[Req], key: u32) -> bool {
    requests
        .iter()
        .any(|r| matches!(r, Req::Key { key: k, state: 1, .. } if *k == key))
}

/// Start in Japanese, focus a text field and type the reading.
fn typed(config: Config, keys: &[u32]) -> Fake {
    let mut fake = Fake::start_japanese(config, &dictionary());
    fake.activate();
    fake.tap_all(keys);
    let _ = fake.settle_panel();
    let _ = fake.take_requests();
    fake
}

#[test]
fn space_opens_the_candidates_and_the_preedit_shows_the_choice() {
    let mut fake = typed(japanese(), &HASHI);
    fake.tap_all(&[SPACE]);
    let shot = fake.wait_for_indicator_shown();
    assert!(shot.width > 48 && shot.height > 48, "a candidate window");
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("橋"));
    assert!(commits(&requests).is_empty());

    fake.tap_all(&[DOWN]);
    let _ = fake.wait_for_indicator_shown();
    assert_eq!(last_preedit(&fake.settle()), Some("箸"));

    fake.tap_all(&[UP, UP]);
    let _ = fake.wait_for_indicator_shown();
    let _ = fake.wait_for_indicator_shown();
    // Up from the first candidate wraps to the last one: the katakana.
    assert_eq!(last_preedit(&fake.settle()), Some("ハシ"));
}

#[test]
fn enter_commits_the_chosen_candidate_once_and_is_not_forwarded() {
    let mut fake = typed(japanese(), &HASHI);
    fake.tap_all(&[SPACE, DOWN]);
    let _ = fake.wait_for_indicator_shown();
    let _ = fake.take_requests();
    fake.tap_all(&[ENTER]);
    let requests = fake.settle();
    assert_eq!(commits(&requests), ["箸"]);
    assert!(!forwarded(&requests, ENTER), "Enter confirmed, not sent");
    fake.wait_for_indicator_hidden();

    // With nothing left to confirm, the next Enter belongs to the application.
    fake.tap_all(&[ENTER]);
    let requests = fake.settle();
    assert!(commits(&requests).is_empty());
    assert!(forwarded(&requests, ENTER));
}

#[test]
fn escape_returns_to_the_reading_and_editing_goes_on() {
    let mut fake = typed(japanese(), &HASHI);
    fake.tap_all(&[SPACE]);
    let _ = fake.wait_for_indicator_shown();
    let _ = fake.take_requests();

    fake.tap_all(&[ESCAPE]);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("はし"));
    assert!(
        commits(&requests).is_empty(),
        "nothing is lost or committed"
    );
    assert!(!forwarded(&requests, ESCAPE));
    fake.wait_for_indicator_hidden();

    // The reading is still editable: Backspace removes し, not the word.
    fake.tap_all(&[BACKSPACE]);
    assert_eq!(last_preedit(&fake.settle()), Some("は"));
}

#[test]
fn backspace_also_returns_to_the_reading_without_deleting() {
    let mut fake = typed(japanese(), &HASHI);
    fake.tap_all(&[SPACE]);
    let _ = fake.wait_for_indicator_shown();
    let _ = fake.take_requests();
    fake.tap_all(&[BACKSPACE]);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("はし"));
    assert!(!forwarded(&requests, BACKSPACE));
    fake.wait_for_indicator_hidden();
}

#[test]
fn page_keys_and_digits_follow_the_page_on_screen() {
    let mut fake = typed(japanese(), &KANA);
    fake.tap_all(&[SPACE]);
    let first = fake.wait_for_indicator_shown();
    assert_eq!(last_preedit(&fake.settle()), Some("案1"));

    fake.tap_all(&[PAGE_DOWN]);
    let second = fake.wait_for_indicator_shown();
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("案10"));
    assert!(!forwarded(&requests, PAGE_DOWN));
    assert_eq!(first.height, second.height, "pages 1 and 2 have 9 rows");

    fake.tap_all(&[PAGE_DOWN]);
    let third = fake.wait_for_indicator_shown();
    assert_eq!(last_preedit(&fake.settle()), Some("案19"));
    assert!(third.height < second.height, "page 3 has only 4 rows");

    fake.tap_all(&[PAGE_UP]);
    let _ = fake.wait_for_indicator_shown();
    assert_eq!(last_preedit(&fake.settle()), Some("案10"));

    // On the second page the window numbers 案10..案18 as 1..9: "2" is 案11.
    fake.tap_all(&[KEY_2]);
    let requests = fake.settle();
    assert_eq!(commits(&requests), ["案11"]);
    assert!(!forwarded(&requests, KEY_2));
    fake.wait_for_indicator_hidden();
}

#[test]
fn words_missing_from_the_dictionary_still_offer_kana() {
    let mut fake = typed(japanese(), &KURUMA);
    fake.tap_all(&[SPACE]);
    let shot = fake.wait_for_indicator_shown();
    assert!(shot.height > 48);
    assert_eq!(last_preedit(&fake.settle()), Some("くるま"));
    fake.tap_all(&[DOWN, ENTER]);
    assert_eq!(commits(&fake.settle()), ["クルマ"]);
}

#[test]
fn switching_language_commits_the_shown_candidate_once() {
    let mut config = japanese();
    config.appearance.indicator_box = false;
    let mut fake = typed(config, &HASHI);
    fake.tap_all(&[SPACE, DOWN]);
    let _ = fake.wait_for_indicator_shown();
    let _ = fake.take_requests();

    fake.tap_all(&[TOGGLE]);
    let requests = fake.settle();
    assert_eq!(commits(&requests), ["箸"], "once, and what was on screen");
    fake.wait_for_indicator_hidden();
}

#[test]
fn losing_focus_keeps_the_shown_candidate_and_closes_the_window() {
    let mut fake = typed(japanese(), &HASHI);
    fake.tap_all(&[SPACE]);
    let _ = fake.wait_for_indicator_shown();
    let _ = fake.take_requests();

    fake.deactivate();
    let requests = fake.take_requests();
    assert_eq!(commits(&requests), ["橋"]);
    fake.wait_for_indicator_hidden();
}

#[test]
fn without_an_input_panel_the_reading_is_kept_and_nothing_is_hidden_from_view() {
    let mut fake = Fake::start_japanese_with_globals(
        japanese(),
        &dictionary(),
        Globals {
            input_panel: false,
            ..Globals::default()
        },
    );
    fake.activate();
    fake.tap_all(&HASHI);
    let _ = fake.take_requests();

    // A conversion nobody could see is never started: the reading stays.
    fake.tap_all(&[SPACE]);
    let requests = fake.settle();
    assert_eq!(last_preedit(&requests), Some("はし"));
    assert!(commits(&requests).is_empty());

    fake.tap_all(&[ENTER]);
    let requests = fake.settle();
    assert_eq!(commits(&requests), ["はし"]);
    assert!(!forwarded(&requests, ENTER));
}

#[test]
fn the_indicator_timer_does_not_close_the_candidate_window() {
    let mut config = japanese();
    config.appearance.indicator_ms = 120;
    let mut fake = Fake::start_japanese(config, &dictionary());
    fake.activate();
    // Arm the indicator timer with a language switch (English, then Japanese).
    for _ in 0..2 {
        fake.tap_all(&[TOGGLE]);
        let _ = fake.wait_for_indicator_shown();
    }
    fake.tap_all(&HASHI);
    let _ = fake.settle_panel();
    fake.tap_all(&[SPACE]);
    let shot = fake.wait_for_indicator_shown();
    assert!(shot.width > 48);
    std::thread::sleep(std::time::Duration::from_millis(220));
    fake.barrier();
    assert!(fake.indicator_mapped(), "the window outlives the timer");
    fake.tap_all(&[ENTER]);
    fake.wait_for_indicator_hidden();
}
