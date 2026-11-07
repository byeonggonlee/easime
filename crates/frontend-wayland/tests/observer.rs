//! The language observer (what feeds the tray icon): told once when serving
//! starts and again on every language change. One test only: the observer is
//! one per process.

mod common;

use common::Fake;
use easime_config::Config;
use ime_core::Language;
use std::sync::mpsc;
use std::time::Duration;

const KEY_HANGEUL: u32 = 122;

#[test]
fn the_observer_hears_the_start_and_every_switch() {
    let (sender, receiver) = mpsc::channel();
    let sender = std::sync::Mutex::new(sender);
    assert!(frontend_wayland::observe_language(
        move |language, cycle| {
            let _ = sender.lock().unwrap().send((language, cycle.to_vec()));
        }
    ));
    assert!(
        !frontend_wayland::observe_language(|_, _| {}),
        "a second observer is refused"
    );

    let mut config = Config::default();
    config.languages.enabled = Some(vec![Language::Korean, Language::Japanese]);
    let mut fake = Fake::start_with(config);
    fake.activate();

    let cycle = vec![Language::English, Language::Korean, Language::Japanese];
    let mut step = 0;
    let mut wait = || {
        step += 1;
        receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("nothing reported at step {step}: {e}"))
    };
    assert_eq!(wait(), (Language::Korean, cycle.clone()), "at the start");

    fake.tap_all(&[KEY_HANGEUL]);
    fake.barrier(); // the client has handled the key
    assert_eq!(wait(), (Language::Japanese, cycle.clone()));
    fake.tap_all(&[KEY_HANGEUL]);
    fake.barrier(); // the client has handled the key
    assert_eq!(wait(), (Language::English, cycle.clone()));
    fake.tap_all(&[KEY_HANGEUL]);
    fake.barrier(); // the client has handled the key
    assert_eq!(wait(), (Language::Korean, cycle));
    assert!(
        receiver.recv_timeout(Duration::from_millis(200)).is_err(),
        "nothing else was reported"
    );
}
