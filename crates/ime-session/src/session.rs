//! The engines for the enabled languages and the rules around them.
//!
//! One [`Session`] is what "the input method is running" means: it knows which
//! language is active, gives a key to that language's engine, and keeps text
//! from being lost when the language, the settings or the focus change. A
//! frontend creates one, feeds it keys and shows what comes back.

use crate::dictionary::JapaneseDictionary;
use easime_config::{BackspaceUnit, Config};
use engine_hangul::{BackspaceMode, HangulEngine};
use engine_japanese::JapaneseEngine;
use ime_core::{EngineOutput, InputEngine, Key, KeyEvent, Language, LanguageSwitcher};
use std::collections::HashMap;

/// Where a session reports what it did (the frontend's own log).
pub type LogFn = Box<dyn Fn(&str) + Send>;

pub struct Session {
    engines: HashMap<Language, Box<dyn InputEngine>>,
    /// Which language is active and what the switch key moves to next.
    switcher: LanguageSwitcher,
    /// Kept for engines built when the settings change.
    dictionary: JapaneseDictionary,
    /// The languages that match the system language, read once: it cannot
    /// change while the process runs.
    system: Vec<Language>,
    log: LogFn,
}

impl Session {
    /// A session for `config`. Languages the user has not chosen follow the
    /// system language (`easime_config::system_languages`).
    pub fn new(config: &Config, dictionary: JapaneseDictionary, log: LogFn) -> Self {
        Self::with_system_languages(config, dictionary, log, &easime_config::system_languages())
    }

    /// Like [`Session::new`], with the languages that match the system language
    /// given instead of read from the environment (tests).
    pub fn with_system_languages(
        config: &Config,
        mut dictionary: JapaneseDictionary,
        log: LogFn,
        system: &[Language],
    ) -> Self {
        let (engines, switcher) = build_engines(config, &mut dictionary, &*log, system);
        Self {
            engines,
            switcher,
            dictionary,
            system: system.to_vec(),
            log,
        }
    }

    /// Switch to new settings while running. Text in progress is committed
    /// first (the returned text, to be inserted), and the current language is
    /// kept if it is still enabled.
    #[must_use = "the text in progress has to be committed"]
    pub fn apply_config(&mut self, config: &Config) -> String {
        let leftover = self.flush();
        let previous = self.switcher.current();
        let last = self.switcher.last();
        let (engines, mut switcher) =
            build_engines(config, &mut self.dictionary, &*self.log, &self.system);
        if let Some(last) = last {
            switcher.remember(last);
        }
        switcher.select(previous);
        self.engines = engines;
        self.switcher = switcher;
        leftover
    }

    /// The active language.
    pub fn language(&self) -> Language {
        self.switcher.current()
    }

    /// The languages the switch key cycles through, English first.
    pub fn cycle(&self) -> &[Language] {
        self.switcher.languages()
    }

    /// The engine of the active language, or `None` for English.
    pub fn engine(&mut self) -> Option<&mut (dyn InputEngine + 'static)> {
        let language = self.switcher.current();
        self.engines.get_mut(&language).map(|e| e.as_mut())
    }

    /// Whether the active language's engine has anything to do with this key
    /// (see [`InputEngine::wants_key`]), without changing anything. English
    /// never does.
    pub fn wants_key(&self, event: KeyEvent) -> bool {
        self.engines
            .get(&self.switcher.current())
            .is_some_and(|engine| engine.wants_key(event))
    }

    /// Give a key to the active language. `None` means English: nothing is
    /// composed and the application gets the key untouched.
    pub fn process_key(&mut self, event: KeyEvent) -> Option<EngineOutput> {
        self.engine().map(|engine| engine.process_key(event))
    }

    /// Finish the text in progress and return it for committing.
    pub fn flush(&mut self) -> String {
        self.engine().map(|e| e.flush()).unwrap_or_default()
    }

    /// Forget everything in progress, in every language (a new text field got
    /// focus).
    pub fn reset(&mut self) {
        for engine in self.engines.values_mut() {
            engine.reset();
        }
    }

    /// What the plain switch key does: go between English and the language
    /// last typed in, or to the next language of the cycle, as the settings say
    /// ([`ime_core::SwitchMode`]). The text in progress is finished first and
    /// returned for committing, so nothing is lost or typed in the wrong
    /// language.
    #[must_use = "the text in progress has to be committed"]
    pub fn switch_language(&mut self) -> (String, Language) {
        let leftover = self.flush();
        let language = self.switcher.toggle();
        (leftover, language)
    }

    /// Move to the next language of the cycle whatever the settings say (Shift
    /// with the switch key). The text in progress is finished first and
    /// returned for committing.
    #[must_use = "the text in progress has to be committed"]
    pub fn next_language(&mut self) -> (String, Language) {
        let leftover = self.flush();
        let language = self.switcher.advance();
        (leftover, language)
    }

    /// Move to `language` (the user chose it, for example from the tray menu).
    /// The text in progress is finished first and returned for committing.
    /// `None` if nothing changed: it is already the active language, or it is
    /// not one of the cycle.
    #[must_use = "the text in progress has to be committed"]
    pub fn select_language(&mut self, language: Language) -> Option<(String, Language)> {
        if self.switcher.current() == language || !self.switcher.languages().contains(&language) {
            return None;
        }
        let leftover = self.flush();
        self.switcher.select(language);
        Some((leftover, language))
    }

    /// The candidate list could not be shown, and a selection the user cannot
    /// see must never be made: take the conversion back as if Escape had been
    /// pressed. The result replaces the output that carried the candidates.
    pub fn cancel_conversion(&mut self) -> Option<EngineOutput> {
        self.process_key(KeyEvent::new(Key::Escape))
    }
}

/// Engines for the enabled languages and the cycle over them.
///
/// Only languages that have an engine take part in the cycle.
fn build_engines(
    config: &Config,
    dictionary: &mut JapaneseDictionary,
    log: &dyn Fn(&str),
    system: &[Language],
) -> (HashMap<Language, Box<dyn InputEngine>>, LanguageSwitcher) {
    let backspace = match config.korean.backspace {
        BackspaceUnit::Jamo => BackspaceMode::Jamo,
        BackspaceUnit::Syllable => BackspaceMode::Syllable,
    };
    let mut engines: HashMap<Language, Box<dyn InputEngine>> = HashMap::new();
    let enabled = config.languages.resolved(system);
    if config.languages.follows_system() {
        log(&format!(
            "languages not chosen in the settings; following the system language: {enabled:?}"
        ));
    } else {
        log(&format!("languages chosen in the settings: {enabled:?}"));
    }
    for &language in &enabled {
        match language {
            Language::Korean => {
                engines.insert(
                    language,
                    Box::new(HangulEngine::with_backspace_mode(backspace)),
                );
            }
            Language::Japanese => {
                engines.insert(
                    language,
                    Box::new(JapaneseEngine::with_dictionary(Box::new(
                        dictionary.get(log),
                    ))),
                );
            }
            Language::English | Language::ChineseSimplified | Language::ChineseTraditional => {
                log(&format!(
                    "language {language:?} is not available yet, ignoring"
                ));
            }
        }
    }
    let usable: Vec<Language> = enabled
        .iter()
        .copied()
        .filter(|l| engines.contains_key(l))
        .collect();
    (
        engines,
        LanguageSwitcher::new(&usable).with_mode(config.languages.switch),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use easime_config::Languages;
    use engine_japanese::Dictionary;
    use engine_japanese::dictionary::MemoryDictionary;
    use std::sync::{Arc, Mutex};

    fn config(languages: Option<Vec<Language>>) -> Config {
        Config {
            languages: Languages {
                enabled: languages,
                ..Languages::default()
            },
            ..Config::default()
        }
    }

    fn dictionary() -> JapaneseDictionary {
        let words: Arc<dyn Dictionary> = Arc::new(MemoryDictionary::parse("にほん\t日本\n").0);
        JapaneseDictionary::fixed(words)
    }

    /// A session and what it logged.
    fn start(config: &Config, system: &[Language]) -> (Session, Arc<Mutex<Vec<String>>>) {
        let messages = Arc::new(Mutex::new(Vec::new()));
        let sink = messages.clone();
        let log: LogFn = Box::new(move |m| sink.lock().unwrap().push(m.to_owned()));
        let session = Session::with_system_languages(config, dictionary(), log, system);
        (session, messages)
    }

    fn press(session: &mut Session, text: &str) -> Option<EngineOutput> {
        let mut last = None;
        for c in text.chars() {
            last = session.process_key(KeyEvent::new(Key::Char(c)));
        }
        last
    }

    #[test]
    fn wants_key_is_about_the_active_language_only() {
        let (mut session, _) = start(&config(Some(vec![Language::Korean])), &[]);
        assert_eq!(
            session.language(),
            Language::Korean,
            "starts on the language"
        );
        assert!(session.wants_key(KeyEvent::ch('g')));
        assert!(!session.wants_key(KeyEvent::ch('1')));
        session.process_key(KeyEvent::ch('g'));
        assert!(
            session.wants_key(KeyEvent::new(Key::Space)),
            "to finish what is composed"
        );
        let _ = session.switch_language();
        assert_eq!(session.language(), Language::English);
        assert!(
            !session.wants_key(KeyEvent::ch('g')),
            "English wants no key"
        );
    }

    #[test]
    fn the_cycle_is_english_then_the_chosen_languages() {
        let chosen = config(Some(vec![Language::Japanese, Language::Korean]));
        let (session, _) = start(&chosen, &[]);
        assert_eq!(
            session.cycle(),
            [Language::English, Language::Japanese, Language::Korean]
        );
        assert_eq!(session.language(), Language::Japanese);
    }

    #[test]
    fn until_chosen_the_system_language_decides() {
        let (session, log) = start(&config(None), &[Language::Japanese]);
        assert_eq!(session.cycle(), [Language::English, Language::Japanese]);
        assert!(log.lock().unwrap()[0].contains("following the system language"));

        let (session, _) = start(&config(None), &[]);
        assert_eq!(session.cycle(), [Language::English]);
        assert_eq!(session.language(), Language::English);
    }

    #[test]
    fn a_choice_wins_over_the_system_language() {
        let (session, log) = start(&config(Some(vec![Language::Korean])), &[Language::Japanese]);
        assert_eq!(session.cycle(), [Language::English, Language::Korean]);
        assert!(log.lock().unwrap()[0].contains("chosen in the settings"));
    }

    #[test]
    fn languages_without_an_engine_stay_out_of_the_cycle_and_are_logged() {
        let chosen = config(Some(vec![Language::ChineseTraditional, Language::Korean]));
        let (session, log) = start(&chosen, &[]);
        assert_eq!(session.cycle(), [Language::English, Language::Korean]);
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .any(|m| m.contains("ChineseTraditional") && m.contains("not available"))
        );
    }

    #[test]
    fn english_hands_the_key_back() {
        let (mut session, _) = start(&config(Some(vec![])), &[]);
        assert!(session.engine().is_none());
        assert_eq!(press(&mut session, "a"), None);
        assert_eq!(session.flush(), "");
    }

    #[test]
    fn keys_go_to_the_active_languages_engine() {
        let (mut session, _) = start(&config(Some(vec![Language::Korean])), &[]);
        let output = press(&mut session, "gk").unwrap();
        assert_eq!(output.preedit.text, "하");
        assert!(output.handled);
    }

    #[test]
    fn switching_language_finishes_the_text_in_progress() {
        let (mut session, _) = start(&config(Some(vec![Language::Korean])), &[]);
        press(&mut session, "gk");
        let (leftover, language) = session.switch_language();
        assert_eq!(leftover, "하");
        assert_eq!(language, Language::English);
        // Back in Korean nothing of it is left to type into.
        let (leftover, language) = session.switch_language();
        assert_eq!((leftover.as_str(), language), ("", Language::Korean));
        assert_eq!(press(&mut session, "g").unwrap().preedit.text, "ㅎ");
    }

    #[test]
    fn a_chosen_language_finishes_the_text_and_becomes_active() {
        let chosen = config(Some(vec![Language::Korean, Language::Japanese]));
        let (mut session, _) = start(&chosen, &[]);
        press(&mut session, "gk");
        let (leftover, language) = session.select_language(Language::Japanese).unwrap();
        assert_eq!((leftover.as_str(), language), ("하", Language::Japanese));
        assert_eq!(session.language(), Language::Japanese);
        // English is part of every cycle.
        let (leftover, language) = session.select_language(Language::English).unwrap();
        assert_eq!((leftover.as_str(), language), ("", Language::English));
        assert!(session.engine().is_none());
    }

    #[test]
    fn choosing_the_active_or_an_unavailable_language_changes_nothing() {
        let (mut session, _) = start(&config(Some(vec![Language::Korean])), &[]);
        press(&mut session, "gk");
        assert_eq!(session.select_language(Language::Korean), None);
        assert_eq!(
            session.select_language(Language::Japanese),
            None,
            "not in the cycle"
        );
        assert_eq!(
            session.select_language(Language::ChineseSimplified),
            None,
            "no engine"
        );
        // Nothing was finished by the refused choices.
        assert_eq!(session.flush(), "하");
    }

    #[test]
    fn new_settings_commit_the_text_and_keep_the_language() {
        let (mut session, _) = start(
            &config(Some(vec![Language::Korean, Language::Japanese])),
            &[],
        );
        press(&mut session, "gk");
        let leftover =
            session.apply_config(&config(Some(vec![Language::Japanese, Language::Korean])));
        assert_eq!(leftover, "하");
        assert_eq!(session.language(), Language::Korean, "still Korean");
        assert_eq!(
            session.cycle(),
            [Language::English, Language::Japanese, Language::Korean]
        );
    }

    #[test]
    fn new_settings_fall_back_when_the_language_is_gone() {
        let (mut session, _) = start(&config(Some(vec![Language::Korean])), &[]);
        let leftover = session.apply_config(&config(Some(vec![Language::Japanese])));
        assert_eq!(leftover, "");
        assert_eq!(session.language(), Language::Japanese);
    }

    #[test]
    fn reset_forgets_the_composition_of_every_language() {
        let (mut session, _) = start(&config(Some(vec![Language::Korean])), &[]);
        press(&mut session, "gk");
        session.reset();
        assert_eq!(session.flush(), "");
    }

    #[test]
    fn a_conversion_that_cannot_be_shown_is_taken_back() {
        let (mut session, _) = start(&config(Some(vec![Language::Korean])), &[]);
        press(&mut session, "gks"); // 한
        let opened = session.process_key(KeyEvent::new(Key::Hanja)).unwrap();
        assert!(
            opened.candidates.is_some(),
            "a Hanja key opens the candidates"
        );
        let taken_back = session.cancel_conversion().unwrap();
        assert!(taken_back.candidates.is_none());
        assert_eq!(taken_back.preedit.text, "한", "the Hangul is still there");
    }

    #[test]
    fn new_settings_for_the_other_options_apply_while_running() {
        let mut syllable = config(Some(vec![Language::Korean]));
        syllable.korean.backspace = easime_config::BackspaceUnit::Syllable;
        let (mut session, _) = start(&config(Some(vec![Language::Korean])), &[]);
        press(&mut session, "gks");
        let _ = session.apply_config(&syllable);
        press(&mut session, "gks");
        let output = session.process_key(KeyEvent::new(Key::Backspace)).unwrap();
        assert_eq!(output.preedit.text, "", "a syllable at a time now");
    }

    #[test]
    fn the_japanese_engine_uses_the_dictionary_given() {
        let (mut session, _) = start(&config(Some(vec![Language::Japanese])), &[]);
        press(&mut session, "nihon");
        let output = session.process_key(KeyEvent::new(Key::Space)).unwrap();
        let list = output.candidates.expect("conversion opens candidates");
        assert!(list.items.contains(&"日本".to_string()), "{:?}", list.items);
    }
}
