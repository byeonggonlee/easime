use engine_hangul::HangulEngine;
use ime_core::{InputEngine, Key, KeyEvent, Modifiers};

fn type_han(engine: &mut HangulEngine) {
    for ch in "gks".chars() {
        engine.process_key(KeyEvent::ch(ch));
    }
}

fn key(engine: &mut HangulEngine, key: Key) -> ime_core::EngineOutput {
    engine.process_key(KeyEvent::new(key))
}

#[test]
fn hanja_opens_candidates_without_committing_the_hangul() {
    let mut engine = HangulEngine::new();
    type_han(&mut engine);
    let out = key(&mut engine, Key::Hanja);
    assert!(out.handled);
    assert!(out.commit.is_empty());
    assert_eq!(out.preedit.text, "한");
    let list = out.candidates.expect("visible Hanja choices");
    assert_eq!(&list.items[..2], ["韓", "漢"]);
    assert_eq!(list.selected, 0);
    assert_eq!(list.annotations[1], "한수 한");
}

#[test]
fn enter_commits_the_selected_hanja_once_and_is_consumed() {
    let mut engine = HangulEngine::new();
    type_han(&mut engine);
    key(&mut engine, Key::Hanja);
    key(&mut engine, Key::Down);
    let out = key(&mut engine, Key::Enter);
    assert_eq!(out.commit, "漢");
    assert!(out.handled);
    assert!(out.preedit.is_empty());
    assert!(out.candidates.is_none());
    assert_eq!(engine.flush(), "");
    assert!(!key(&mut engine, Key::Enter).handled);
}

#[test]
fn escape_restores_the_original_jamo_editing_state() {
    let mut engine = HangulEngine::new();
    type_han(&mut engine);
    key(&mut engine, Key::Hanja);
    key(&mut engine, Key::Down);
    let out = key(&mut engine, Key::Escape);
    assert!(out.handled && out.commit.is_empty() && out.candidates.is_none());
    assert_eq!(out.preedit.text, "한");
    assert_eq!(key(&mut engine, Key::Backspace).preedit.text, "하");
    assert_eq!(engine.process_key(KeyEvent::ch('s')).preedit.text, "한");
}

#[test]
fn unavailable_candidates_preserve_the_composition() {
    let mut engine = HangulEngine::new();
    engine.process_key(KeyEvent::ch('R'));
    let out = key(&mut engine, Key::Hanja);
    assert!(out.handled && out.commit.is_empty() && out.candidates.is_none());
    assert_eq!(out.preedit.text, "ㄲ");
    assert_eq!(engine.process_key(KeyEvent::ch('k')).preedit.text, "까");
}

#[test]
fn idle_hanja_key_does_not_consume_the_application_key() {
    let mut engine = HangulEngine::new();
    assert!(!key(&mut engine, Key::Hanja).handled);
}

#[test]
fn page_numbers_select_from_the_current_page() {
    let mut engine = HangulEngine::new();
    type_han(&mut engine);
    let list = key(&mut engine, Key::Hanja).candidates.unwrap();
    assert!(list.items.len() > 9);
    let page = key(&mut engine, Key::PageDown).candidates.unwrap();
    assert_eq!(page.selected, 9);
    let out = engine.process_key(KeyEvent::ch('2'));
    assert_eq!(out.commit, list.items[10]);
    assert!(out.handled);
}

#[test]
fn page_up_returns_to_the_first_page() {
    let mut engine = HangulEngine::new();
    type_han(&mut engine);
    key(&mut engine, Key::Hanja);
    key(&mut engine, Key::PageDown);
    assert_eq!(
        key(&mut engine, Key::PageUp).candidates.unwrap().selected,
        0
    );
}

#[test]
fn focus_or_language_flush_preserves_unconfirmed_hangul() {
    let mut engine = HangulEngine::new();
    type_han(&mut engine);
    key(&mut engine, Key::Hanja);
    key(&mut engine, Key::Down);
    assert_eq!(engine.flush(), "한");
    assert_eq!(engine.flush(), "");
    assert!(!key(&mut engine, Key::Enter).handled);
}

#[test]
fn shortcuts_never_commit_an_unconfirmed_candidate() {
    let mut engine = HangulEngine::new();
    type_han(&mut engine);
    key(&mut engine, Key::Hanja);
    let out = engine.process_key(KeyEvent {
        key: Key::Char('c'),
        modifiers: Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    });
    assert_eq!(out.commit, "한");
    assert!(!out.handled);
    assert!(out.candidates.is_none());
}

#[test]
fn typing_after_cancellation_continues_the_original_syllable() {
    let mut engine = HangulEngine::new();
    type_han(&mut engine);
    key(&mut engine, Key::Hanja);
    let out = engine.process_key(KeyEvent::ch('k'));
    assert_eq!(out.commit, "하");
    assert_eq!(out.preedit.text, "나");
    assert!(out.candidates.is_none());
}

#[test]
fn reset_clears_both_candidates_and_composition() {
    let mut engine = HangulEngine::new();
    type_han(&mut engine);
    key(&mut engine, Key::Hanja);
    engine.reset();
    assert_eq!(engine.flush(), "");
    assert!(!key(&mut engine, Key::Hanja).handled);
}

#[test]
fn repeated_hanja_does_not_advance_or_commit_a_candidate() {
    let mut engine = HangulEngine::new();
    type_han(&mut engine);
    let first = key(&mut engine, Key::Hanja);
    let repeated = key(&mut engine, Key::Hanja);
    assert_eq!(first, repeated);
}
