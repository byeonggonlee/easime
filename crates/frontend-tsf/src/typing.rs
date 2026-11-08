//! What the text service does with a key, without any Windows API (so it is
//! tested on every OS): which keys it takes, what text and composition result,
//! and when the language switches.
//!
//! TSF asks about every key twice: `OnTestKeyDown` ("would you take it?", no
//! side effects) and then `OnKeyDown` (take it). [`Typist::test_key_down`] and
//! [`Typist::key_down`] are those two, and they agree: a key the test refuses is
//! never touched by the second, and a key the second takes was accepted by the
//! first (checked for every key in the tests). The result of a key is an
//! [`Outcome`]: whether the application is to have the key, and the [`Edit`] to
//! make in the document first. [`plan`] turns an edit into the few operations
//! the Windows code performs on the document ([`Step`]).
//!
//! Where Windows differs from the Wayland frontend (`docs/platforms.md`):
//!
//! * The right Alt press is **not** held back and replayed later; it goes to
//!   the application at once, and only a *tap* (released with no other key in
//!   between) is taken, at the release. TSF cannot send a held-back key again,
//!   and an application must see Alt when it is the modifier of a shortcut.
//! * Holding the Hangul key does not switch again and again.
//! * There is no candidate window yet, so a conversion is taken back at once:
//!   a selection nobody can see is never made.

use crate::keys::{self, KeyInfo, KeyboardState};
use crate::marks::{Mark, marks};
use easime_config::ToggleKey;
use ime_core::{Key, Language};
use ime_session::{Decision, KeyRole, Session, ToggleTracker};
use std::collections::HashSet;
use std::ops::Range;

/// What to change in the document for one key.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Edit {
    /// Finished text to put into the document (before anything else).
    pub commit: String,
    /// The composition that follows the commit; empty ends the composition.
    pub preedit: String,
    /// How the parts of the composition are drawn, in UTF-16 units of
    /// `preedit` ([`crate::marks`]).
    pub marks: Vec<(Range<usize>, Mark)>,
}

impl Edit {
    pub fn is_empty(&self) -> bool {
        self.commit.is_empty() && self.preedit.is_empty()
    }
}

/// The result of a key.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    /// The key is easime's: the application must not get it. (When false, the
    /// application gets the key *after* the edit.)
    pub eaten: bool,
    /// What to change in the document before that; `None` leaves it alone.
    pub edit: Option<Edit>,
    /// Set when the key switched the language.
    pub switched_to: Option<Language>,
}

impl Outcome {
    fn passed() -> Self {
        Self::default()
    }

    fn eaten() -> Self {
        Self {
            eaten: true,
            ..Self::default()
        }
    }
}

/// One operation on the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Make this the text of the composition (starting one at the cursor if
    /// there is none) and put the cursor after it.
    Text(String),
    /// The composition is over; its text stays in the document as it is.
    End,
}

/// The operations that make `edit` happen when a composition is (`composing`)
/// or is not going on. Finished text is written into the composition and the
/// composition ended, so it is inserted exactly where the composition was.
pub fn plan(composing: bool, edit: &Edit) -> Vec<Step> {
    let mut steps = Vec::new();
    let mut composing = composing;
    if !edit.commit.is_empty() {
        steps.push(Step::Text(edit.commit.clone()));
        steps.push(Step::End);
        composing = false;
    }
    if !edit.preedit.is_empty() {
        steps.push(Step::Text(edit.preedit.clone()));
    } else if composing {
        steps.push(Step::Text(String::new()));
        steps.push(Step::End);
    }
    steps
}

/// The input method of one thread of one application.
pub struct Typist {
    session: Session,
    toggle_key: ToggleKey,
    toggle: ToggleTracker<u16, ()>,
    /// Keys whose press was taken, so their release is taken too.
    taken_presses: HashSet<u16>,
    /// The composition last asked for (what the document shows as composing).
    shown: String,
}

impl Typist {
    pub fn new(session: Session, toggle_key: ToggleKey) -> Self {
        Self {
            session,
            toggle_key,
            toggle: ToggleTracker::with_setting(toggle_key),
            taken_presses: HashSet::new(),
            shown: String::new(),
        }
    }

    /// The active language.
    pub fn language(&self) -> Language {
        self.session.language()
    }

    /// The composition the document is showing.
    pub fn shown(&self) -> &str {
        &self.shown
    }

    /// `OnTestKeyDown`: would the key be taken? Changes nothing.
    pub fn test_key_down(&self, key: KeyInfo, state: &KeyboardState) -> bool {
        match keys::toggle_role(key) {
            KeyRole::HangulKey if self.toggle_key.uses_hangul_key() => return true,
            // The press goes to the application; the tap is taken on release.
            KeyRole::RightAlt if self.toggle_key.uses_right_alt() => return false,
            _ => {}
        }
        let Some(event) = keys::key_event(key, state) else {
            return false;
        };
        event.key != Key::Hanja && self.session.wants_key(event)
    }

    /// `OnKeyDown`: take the key (or not) and say what to do with the document.
    pub fn key_down(&mut self, key: KeyInfo, state: &KeyboardState) -> Outcome {
        let role = match keys::toggle_role(key) {
            KeyRole::RightAlt
                if keys::shift_held(state)
                    || keys::modifiers(state).ctrl
                    || keys::modifiers(state).meta =>
            {
                KeyRole::Other
            }
            role => role,
        };
        if key.repeat && role == KeyRole::HangulKey && self.toggle_key.uses_hangul_key() {
            // Held down: one press switched, the repeats are not more presses.
            return Outcome::eaten();
        }
        match self.toggle.on_press(key.vk, role, ()).1 {
            Decision::Toggle => {
                self.taken_presses.insert(key.vk);
                return self.switch();
            }
            // The right Alt press is not held back: see the module docs.
            Decision::Swallow => return Outcome::passed(),
            Decision::Pass => {}
        }
        let Some(event) = keys::key_event(key, state) else {
            return Outcome::passed();
        };
        if event.key == Key::Hanja {
            // Conversion needs a candidate window, which does not exist yet.
            return Outcome::passed();
        }
        let Some(mut output) = self.session.process_key(event) else {
            // English: the application gets the key untouched.
            return Outcome::passed();
        };
        let handled = output.handled;
        if output.candidates.is_some() {
            // Never allow a selection that the user could not see.
            if let Some(taken_back) = self.session.cancel_conversion() {
                output = taken_back;
            }
        }
        let was_showing = !self.shown.is_empty();
        self.shown = output.preedit.text.clone();
        let edit = Edit {
            marks: marks(self.language(), &output.preedit),
            commit: output.commit,
            preedit: output.preedit.text,
        };
        Outcome {
            eaten: handled,
            // Nothing to say and nothing on screen to take away: leave the document alone.
            edit: (was_showing || !edit.is_empty()).then_some(edit),
            switched_to: None,
        }
    }

    /// `OnTestKeyUp`: would the release be taken? Changes nothing.
    pub fn test_key_up(&self, key: KeyInfo) -> bool {
        self.taken_presses.contains(&key.vk) || self.toggle.holds(key.vk)
    }

    /// `OnKeyUp`.
    pub fn key_up(&mut self, key: KeyInfo) -> Outcome {
        if self.taken_presses.remove(&key.vk) {
            return Outcome::eaten();
        }
        match self.toggle.on_release(key.vk) {
            Decision::Toggle => self.switch(),
            _ => Outcome::passed(),
        }
    }

    /// The switch key was used: finish what is being composed, then the next
    /// language.
    fn switch(&mut self) -> Outcome {
        let (leftover, language) = self.session.switch_language();
        Outcome {
            eaten: true,
            edit: self.finished(leftover),
            switched_to: Some(language),
        }
    }

    /// The edit that finishes the composition on screen with `leftover` as its
    /// final text, if there is anything to finish.
    fn finished(&mut self, leftover: String) -> Option<Edit> {
        let was_showing = !self.shown.is_empty();
        self.shown.clear();
        (was_showing || !leftover.is_empty()).then(|| Edit {
            commit: leftover,
            ..Edit::default()
        })
    }

    /// Choose the language directly (the profile the user picked in Windows).
    pub fn select_language(&mut self, language: Language) -> Outcome {
        match self.session.select_language(language) {
            Some((leftover, language)) => Outcome {
                eaten: true,
                edit: self.finished(leftover),
                switched_to: Some(language),
            },
            None => Outcome::passed(),
        }
    }

    /// The composition is over, whether we ended it (the text field lost the
    /// focus) or the application did: drop the engine's copy. The document
    /// keeps whatever text it shows, which is what the composition was.
    pub fn forget(&mut self) {
        self.toggle.clear();
        self.taken_presses.clear();
        self.session.reset();
        self.shown.clear();
    }

    /// New settings while running: what is being composed is finished first
    /// (the edit to make in the document, if any).
    pub fn apply_config(&mut self, config: &easime_config::Config) -> Option<Edit> {
        let leftover = self.session.apply_config(config);
        self.toggle_key = config.shortcuts.toggle_key;
        self.toggle.set_setting(self.toggle_key);
        self.finished(leftover)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{VK_BACK, VK_CONTROL, VK_ESCAPE, VK_HANGUL, VK_HANJA, VK_MENU, VK_RETURN};
    use crate::keys::{VK_RMENU, VK_SHIFT, VK_SPACE};
    use crate::profiles::effective_config;
    use easime_config::{Config, Languages};
    use ime_session::JapaneseDictionary;

    const NO_KEYS: KeyboardState = [0; 256];

    fn config(language: Language) -> Config {
        Config {
            languages: Languages {
                enabled: Some(vec![language]),
            },
            ..Config::default()
        }
    }

    fn korean() -> Typist {
        let session = Session::new(
            &config(Language::Korean),
            JapaneseDictionary::installed(),
            Box::new(|_| {}),
        );
        Typist::new(session, ToggleKey::Both)
    }

    /// The physical key of a letter on a US keyboard.
    fn letter(c: char) -> KeyInfo {
        let scan = [
            ("qwertyuiop", 0x10u8),
            ("asdfghjkl", 0x1E),
            ("zxcvbnm", 0x2C),
        ]
        .into_iter()
        .find_map(|(row, first)| {
            row.find(c.to_ascii_lowercase())
                .map(|i| first + u8::try_from(i).unwrap())
        })
        .expect("a letter");
        KeyInfo {
            vk: u16::from(c.to_ascii_uppercase() as u8),
            scan,
            extended: false,
            repeat: false,
        }
    }

    fn named(vk: u16, scan: u8) -> KeyInfo {
        KeyInfo {
            vk,
            scan,
            extended: false,
            repeat: false,
        }
    }

    fn right_alt() -> KeyInfo {
        KeyInfo {
            extended: true,
            ..named(VK_MENU, 0x38)
        }
    }

    fn held(vks: &[u16]) -> KeyboardState {
        let mut state = NO_KEYS;
        for &vk in vks {
            state[usize::from(vk)] = 0x80;
        }
        state
    }

    /// An application's text field with the composition on it, and the user
    /// typing: what TSF does with the answers of the typist.
    struct Field {
        typist: Typist,
        /// Everything in the field.
        text: String,
        /// How many chars at the end are the composition.
        composing: usize,
    }

    impl Field {
        fn new(typist: Typist) -> Self {
            Self {
                typist,
                text: String::new(),
                composing: 0,
            }
        }

        fn apply(&mut self, edit: &Option<Edit>) {
            let Some(edit) = edit else {
                return;
            };
            for step in plan(self.composing > 0, edit) {
                match step {
                    Step::Text(text) => {
                        let keep = self.text.chars().count() - self.composing;
                        self.text = self.text.chars().take(keep).collect::<String>() + &text;
                        self.composing = text.chars().count();
                    }
                    Step::End => self.composing = 0,
                }
            }
        }

        /// Press and release a key like TSF does: ask, then act. `typed` is
        /// what the application inserts when it gets the key.
        fn press(&mut self, key: KeyInfo, state: &KeyboardState, typed: Option<char>) -> bool {
            let asked = self.typist.test_key_down(key, state);
            if !asked {
                // Not wanted: the application has it, and nothing else happens.
                self.insert(typed);
                return false;
            }
            let outcome = self.typist.key_down(key, state);
            self.apply(&outcome.edit);
            if !outcome.eaten {
                self.insert(typed);
            }
            let released = self.typist.test_key_up(key);
            let up = self.typist.key_up(key);
            assert_eq!(
                released, up.eaten,
                "asking about the release agrees with the release"
            );
            self.apply(&up.edit);
            outcome.eaten
        }

        fn insert(&mut self, typed: Option<char>) {
            if let Some(c) = typed {
                self.text.push(c);
                self.composing = 0;
            }
        }

        fn type_letters(&mut self, text: &str) {
            for c in text.chars() {
                let shift = c.is_ascii_uppercase();
                let state = if shift { held(&[VK_SHIFT]) } else { NO_KEYS };
                self.press(letter(c), &state, Some(c));
            }
        }
    }

    #[test]
    fn dubeolsik_letters_become_hangul_with_the_last_syllable_composing() {
        let mut field = Field::new(korean());
        field.type_letters("gksrmf");
        assert_eq!(field.text, "한글");
        assert_eq!(
            field.composing, 1,
            "only the last syllable is still composing"
        );
        assert_eq!(field.typist.shown(), "글");
    }

    #[test]
    fn the_composition_says_how_it_is_drawn_and_finished_text_does_not() {
        let mut typist = korean();
        let edit = typist.key_down(letter('g'), &NO_KEYS).edit.unwrap();
        assert_eq!(edit.preedit, "ㅎ");
        assert_eq!(edit.marks, [(0..1, Mark::Composing)]);
        let edit = typist.key_down(letter('k'), &NO_KEYS).edit.unwrap();
        assert_eq!(edit.marks, [(0..1, Mark::Composing)]);
        // Space finishes the syllable: finished text carries no mark.
        let edit = typist
            .key_down(named(VK_SPACE, 0x39), &NO_KEYS)
            .edit
            .unwrap();
        assert_eq!((edit.commit.as_str(), edit.preedit.as_str()), ("하", ""));
        assert!(edit.marks.is_empty());
    }

    #[test]
    fn space_finishes_the_syllable_and_the_application_gets_the_space() {
        let mut field = Field::new(korean());
        field.type_letters("gk");
        let eaten = field.press(named(VK_SPACE, 0x39), &NO_KEYS, Some(' '));
        assert!(!eaten, "the application inserts the space itself");
        assert_eq!(field.text, "하 ");
        assert_eq!(field.composing, 0);
    }

    #[test]
    fn backspace_edits_the_composition_then_belongs_to_the_application() {
        let mut field = Field::new(korean());
        field.type_letters("gks");
        assert!(field.press(named(VK_BACK, 0x0E), &NO_KEYS, None));
        assert_eq!(field.text, "하");
        assert!(field.press(named(VK_BACK, 0x0E), &NO_KEYS, None));
        assert!(field.press(named(VK_BACK, 0x0E), &NO_KEYS, None));
        assert_eq!(field.text, "", "the composition is gone with its last jamo");
        assert!(
            !field.press(named(VK_BACK, 0x0E), &NO_KEYS, None),
            "nothing composing: the application deletes"
        );
    }

    #[test]
    fn a_shortcut_finishes_the_syllable_first() {
        let mut field = Field::new(korean());
        field.type_letters("gk");
        let ctrl = held(&[VK_CONTROL]);
        let eaten = field.press(letter('c'), &ctrl, None);
        assert!(!eaten, "Ctrl+C is the application's");
        assert_eq!(field.text, "하");
        assert_eq!(field.composing, 0);
    }

    #[test]
    fn digits_and_enter_finish_the_syllable_and_go_on() {
        let mut field = Field::new(korean());
        field.type_letters("gk");
        let one = named(0x31, 0x02);
        assert!(!field.press(one, &NO_KEYS, Some('1')));
        assert_eq!(field.text, "하1");
        field.type_letters("gk");
        assert!(!field.press(named(VK_RETURN, 0x1C), &NO_KEYS, Some('\n')));
        assert_eq!(field.text, "하1하\n");
    }

    #[test]
    fn the_hangul_key_switches_to_english_and_back_and_is_never_seen_by_the_application() {
        let mut field = Field::new(korean());
        field.type_letters("gk");
        let hangul = named(VK_HANGUL, 0x72);
        assert!(field.press(hangul, &NO_KEYS, None));
        assert_eq!(field.typist.language(), Language::English);
        assert_eq!(field.text, "하", "what was composing is finished first");
        assert_eq!(field.composing, 0);
        field.type_letters("gk");
        assert_eq!(field.text, "하gk", "English passes the keys on");
        assert!(field.press(hangul, &NO_KEYS, None));
        assert_eq!(field.typist.language(), Language::Korean);
        field.type_letters("gk");
        assert_eq!(field.text, "하gk하");
    }

    #[test]
    fn one_key_goes_english_korean_japanese_english_in_one_profile() {
        use engine_japanese::dictionary::MemoryDictionary;
        let words = std::sync::Arc::new(MemoryDictionary::parse("にほん\t日本\n").0);
        let mut chosen = Config::default();
        chosen.languages.enabled = Some(vec![Language::Korean, Language::Japanese]);
        // What the Windows service builds from the settings once Japanese can be typed.
        let config = effective_config(
            &chosen,
            &[],
            &[Language::Korean, Language::Japanese],
            Language::Korean,
        );
        let session = Session::new(&config, JapaneseDictionary::fixed(words), Box::new(|_| {}));
        let mut field = Field::new(Typist::new(session, ToggleKey::Both));
        let hangul = named(VK_HANGUL, 0x72);
        assert_eq!(
            field.typist.language(),
            Language::Korean,
            "starts typing Korean"
        );
        field.type_letters("gk");
        assert_eq!(field.text, "하");

        // Korean -> Japanese: what was being composed is finished first.
        assert!(field.press(hangul, &NO_KEYS, None));
        assert_eq!(field.typist.language(), Language::Japanese);
        assert_eq!(field.text, "하");
        field.type_letters("nihon");
        assert_eq!(field.text, "하にほn");

        // Japanese -> English: the kana are finished, the next keys are the application's.
        assert!(field.press(hangul, &NO_KEYS, None));
        assert_eq!(field.typist.language(), Language::English);
        assert_eq!(field.text, "하にほん");
        field.type_letters("gk");
        assert_eq!(field.text, "하にほんgk");

        // English -> Korean again.
        assert!(field.press(hangul, &NO_KEYS, None));
        assert_eq!(field.typist.language(), Language::Korean);
        field.type_letters("gk");
        assert_eq!(field.text, "하にほんgk하");
    }

    #[test]
    fn holding_the_hangul_key_switches_once() {
        let mut typist = korean();
        let hangul = named(VK_HANGUL, 0x72);
        assert!(typist.key_down(hangul, &NO_KEYS).eaten);
        assert_eq!(typist.language(), Language::English);
        let repeat = KeyInfo {
            repeat: true,
            ..hangul
        };
        for _ in 0..5 {
            let outcome = typist.key_down(repeat, &NO_KEYS);
            assert!(outcome.eaten && outcome.switched_to.is_none());
        }
        assert_eq!(typist.language(), Language::English);
        assert!(typist.test_key_up(hangul));
        assert!(typist.key_up(hangul).eaten);
        assert!(!typist.test_key_up(hangul), "only one release to take");
    }

    #[test]
    fn tapping_right_alt_switches_on_release_and_the_press_goes_to_the_application() {
        let mut typist = korean();
        let alt = right_alt();
        assert!(!typist.test_key_down(alt, &NO_KEYS));
        let press = typist.key_down(alt, &NO_KEYS);
        assert!(
            !press.eaten,
            "the application sees the press: it may be a modifier"
        );
        assert_eq!(typist.language(), Language::Korean);
        assert!(typist.test_key_up(alt));
        let release = typist.key_up(alt);
        assert!(release.eaten);
        assert_eq!(release.switched_to, Some(Language::English));
    }

    #[test]
    fn right_alt_used_as_a_modifier_does_not_switch() {
        let mut typist = korean();
        let alt = right_alt();
        typist.key_down(alt, &NO_KEYS);
        typist.key_down(letter('f'), &held(&[VK_MENU]));
        assert!(!typist.test_key_up(alt));
        let release = typist.key_up(alt);
        assert!(!release.eaten && release.switched_to.is_none());
        assert_eq!(typist.language(), Language::Korean);

        // AltGr (Ctrl already down when right Alt is pressed) or Shift+RightAlt
        // must not arm the switch either.
        for modifier in [VK_CONTROL, VK_SHIFT] {
            typist.key_down(alt, &held(&[modifier]));
            assert!(!typist.test_key_up(alt));
            let release = typist.key_up(alt);
            assert!(!release.eaten && release.switched_to.is_none());
            assert_eq!(typist.language(), Language::Korean);
        }
    }

    #[test]
    fn the_setting_chooses_the_switch_keys() {
        let session = |language| {
            Session::new(
                &config(language),
                JapaneseDictionary::installed(),
                Box::new(|_| {}),
            )
        };
        let mut hangul_only = Typist::new(session(Language::Korean), ToggleKey::Hangul);
        hangul_only.key_down(right_alt(), &NO_KEYS);
        assert!(
            !hangul_only.test_key_up(right_alt()),
            "right Alt is not a switch key"
        );
        assert!(!hangul_only.key_up(right_alt()).eaten);

        let mut alt_only = Typist::new(session(Language::Korean), ToggleKey::RightAlt);
        let hangul = named(VK_HANGUL, 0x72);
        assert!(!alt_only.test_key_down(hangul, &NO_KEYS));
        assert!(!alt_only.key_down(hangul, &NO_KEYS).eaten);
        assert_eq!(alt_only.language(), Language::Korean);
    }

    #[test]
    fn the_hanja_key_is_left_alone_while_there_is_no_candidate_window() {
        let mut field = Field::new(korean());
        field.type_letters("gk");
        let hanja = named(VK_HANJA, 0x73);
        assert!(!field.typist.test_key_down(hanja, &NO_KEYS));
        assert!(!field.press(hanja, &NO_KEYS, None));
        assert_eq!(field.text, "하");
        assert_eq!(field.composing, 1, "the composition is intact");
    }

    #[test]
    fn a_conversion_that_cannot_be_shown_is_taken_back() {
        use engine_japanese::dictionary::MemoryDictionary;
        let words = std::sync::Arc::new(MemoryDictionary::parse("にほん\t日本,二本\n").0);
        let session = Session::new(
            &config(Language::Japanese),
            JapaneseDictionary::fixed(words),
            Box::new(|_| {}),
        );
        let mut field = Field::new(Typist::new(session, ToggleKey::Both));
        field.type_letters("nihon");
        let eaten = field.press(named(VK_SPACE, 0x39), &NO_KEYS, Some(' '));
        assert!(eaten, "Space was for the conversion");
        assert_eq!(field.text, "にほん", "back to the reading, not a candidate");
        assert_eq!(field.composing, 3);
        // Escape was the key the conversion was taken back with: the next one is the user's.
        assert!(field.press(named(VK_ESCAPE, 0x01), &NO_KEYS, None));
        assert_eq!(field.text, "");
    }

    #[test]
    fn english_takes_no_key() {
        let mut typist = korean();
        typist.key_down(named(VK_HANGUL, 0x72), &NO_KEYS);
        assert_eq!(typist.language(), Language::English);
        for c in "gksrmf".chars() {
            assert!(!typist.test_key_down(letter(c), &NO_KEYS));
            let outcome = typist.key_down(letter(c), &NO_KEYS);
            assert!(!outcome.eaten && outcome.edit.is_none());
        }
    }

    /// Keys of every kind: letters, digits, the named keys, the switch keys and
    /// the modifiers; with Shift, Ctrl, Alt and nothing held.
    fn many_keys() -> Vec<(KeyInfo, KeyboardState)> {
        let mut vks: Vec<u16> = (0x41..=0x5A).chain(0x30..=0x39).collect();
        vks.extend([
            VK_BACK, 0x09, VK_RETURN, VK_SHIFT, VK_CONTROL, VK_MENU, VK_RMENU, VK_HANGUL, VK_HANJA,
            VK_ESCAPE, VK_SPACE, 0x21, 0x22, 0x25, 0x26, 0x27, 0x28, 0x14, 0x5B, 0x70, 0x60, 0x6A,
            0xBA, 0xBC,
        ]);
        let states = [
            NO_KEYS,
            held(&[VK_SHIFT]),
            held(&[VK_CONTROL]),
            held(&[VK_MENU]),
        ];
        let mut keys = Vec::new();
        for &vk in &vks {
            for scan in [0x02u8, 0x10, 0x1E, 0x22, 0x25, 0x2C, 0x35, 0x38, 0x39, 0x48] {
                for extended in [false, true] {
                    keys.push((
                        KeyInfo {
                            vk,
                            scan,
                            extended,
                            repeat: false,
                        },
                        NO_KEYS,
                    ));
                }
            }
        }
        let mut with_state = Vec::new();
        for (key, _) in keys.iter().step_by(3) {
            for state in states {
                with_state.push((*key, state));
            }
        }
        with_state
    }

    #[test]
    fn asking_and_acting_agree_for_every_key_in_every_state() {
        // States worth being in: idle, composing a syllable, English.
        let prepare: [fn(&mut Typist); 4] = [
            |_| {},
            |t| {
                t.key_down(letter('g'), &NO_KEYS);
            },
            |t| {
                for c in "gks".chars() {
                    t.key_down(letter(c), &NO_KEYS);
                }
            },
            |t| {
                t.key_down(named(VK_HANGUL, 0x72), &NO_KEYS);
            },
        ];
        for prepare in prepare {
            for (key, state) in many_keys() {
                let mut asked = korean();
                prepare(&mut asked);
                let mut plain = korean();
                prepare(&mut plain);
                let wanted = asked.test_key_down(key, &state);
                // Asking must not change anything.
                let _ = asked.test_key_up(key);
                let acted = asked.key_down(key, &state);
                let direct = plain.key_down(key, &state);
                assert_eq!(acted, direct, "asking changed the result for {key:?}");
                if !wanted {
                    assert!(
                        !acted.eaten && acted.edit.is_none() && acted.switched_to.is_none(),
                        "the test refused {key:?} but the key did {acted:?}"
                    );
                }
                if acted.eaten {
                    assert!(wanted, "{key:?} was eaten although the test refused it");
                }
            }
        }
    }

    #[test]
    fn forgetting_drops_the_engines_copy() {
        let mut field = Field::new(korean());
        field.type_letters("gk");
        // The application ended the composition itself: its text stays.
        field.typist.forget();
        field.composing = 0;
        assert_eq!(field.text, "하");
        assert_eq!(field.typist.shown(), "");
        field.type_letters("g");
        assert_eq!(
            field.text, "하ㅎ",
            "a new syllable starts, nothing of the old one is left"
        );
    }

    #[test]
    fn new_settings_finish_the_composition() {
        let mut field = Field::new(korean());
        field.type_letters("gk");
        let edit = field.typist.apply_config(&config(Language::Korean));
        assert!(edit.is_some());
        field.apply(&edit);
        assert_eq!(field.text, "하");
        assert_eq!(field.composing, 0);
        field.type_letters("gk");
        assert_eq!(field.text, "하하");
    }

    #[test]
    fn the_language_can_be_chosen_directly() {
        let mut field = Field::new(korean());
        field.type_letters("gk");
        let outcome = field.typist.select_language(Language::English);
        field.apply(&outcome.edit);
        assert_eq!(outcome.switched_to, Some(Language::English));
        assert_eq!(field.text, "하");
        assert_eq!(field.composing, 0);
        assert_eq!(
            field.typist.select_language(Language::English),
            Outcome::default(),
            "already there"
        );
    }

    #[test]
    fn the_plan_for_every_case() {
        let edit = |commit: &str, preedit: &str| Edit {
            commit: commit.into(),
            preedit: preedit.into(),
            ..Edit::default()
        };
        let text = |t: &str| Step::Text(t.into());
        assert_eq!(plan(false, &edit("", "")), []);
        assert_eq!(plan(false, &edit("", "하")), [text("하")]);
        assert_eq!(plan(false, &edit("한", "")), [text("한"), Step::End]);
        assert_eq!(
            plan(false, &edit("한", "글")),
            [text("한"), Step::End, text("글")]
        );
        assert_eq!(plan(true, &edit("", "하")), [text("하")]);
        assert_eq!(plan(true, &edit("", "")), [text(""), Step::End]);
        assert_eq!(plan(true, &edit("한", "")), [text("한"), Step::End]);
        assert_eq!(
            plan(true, &edit("한", "글")),
            [text("한"), Step::End, text("글")]
        );
    }
}
