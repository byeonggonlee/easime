//! What easime registers with Windows: one text service with **one profile**
//! (decision of 2026-11-08, `docs/decisions.md`; it replaced "one profile per
//! language" the same day).
//!
//! A Windows profile is listed under exactly one language, the *home* language
//! ([`HOME`], Korean). Inside it easime cycles through the languages the user
//! chose with the one switch key, English, Korean, Japanese, English, ... like
//! the Linux version, so nobody has to add several Windows languages by hand and
//! the cycle and the settings are the same everywhere. The Windows language
//! indicator keeps showing the home language whichever one is typed; the box next
//! to the cursor says which one is on.
//!
//! Only languages the text service can really type are in the cycle
//! ([`TYPEABLE`]), so Windows never offers an input method that does nothing
//! (DESIGN.md 3.1). The GUIDs are fixed for ever: Windows remembers the user's
//! choice by them.

use easime_config::Config;
use ime_core::Language;

/// The class id of the easime text service.
pub const CLSID: &str = "{CFC09673-DDBA-4C84-B57F-CF0B0DDC262C}";

/// One input method in Windows' language list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// The language whose engine it runs.
    pub language: Language,
    /// The Windows language id (LANGID) the profile is listed under.
    pub lang_id: u16,
    /// The profile's id.
    pub guid: &'static str,
}

impl Profile {
    /// The name Windows shows for it.
    pub fn name(&self) -> String {
        "easime".to_owned()
    }
}

/// The language the one profile is listed under in Windows' language settings.
pub const HOME: Language = Language::Korean;

/// A profile for each language Windows could list easime under. Only the
/// [`HOME`] one is registered ([`registered`]); the others are kept so that
/// removing easime also removes anything an earlier build registered.
pub const ALL: [Profile; 4] = [
    Profile {
        language: Language::Korean,
        lang_id: 0x0412, // ko-KR
        guid: "{42FBAB7A-5B91-4A6A-868A-3CC5C8153492}",
    },
    Profile {
        language: Language::Japanese,
        lang_id: 0x0411, // ja-JP
        guid: "{C059F0B3-8110-4CA5-AF4F-B2C79C3BE921}",
    },
    Profile {
        language: Language::ChineseTraditional,
        lang_id: 0x0404, // zh-TW
        guid: "{25E11A69-7BDC-4491-B31D-3060F63CA7D4}",
    },
    Profile {
        language: Language::ChineseSimplified,
        lang_id: 0x0804, // zh-CN
        guid: "{F2EC3B81-9A21-4B5C-BD2D-D6D776A9DC72}",
    },
];

/// The profile of `language`, if easime has one.
pub fn profile_of(language: Language) -> Option<&'static Profile> {
    ALL.iter().find(|profile| profile.language == language)
}

/// A language by the name the installer passes (`korean`, `japanese`, ...).
pub fn language_named(name: &str) -> Option<Language> {
    match name.trim().to_ascii_lowercase().as_str() {
        "korean" => Some(Language::Korean),
        "japanese" => Some(Language::Japanese),
        "chinese_simplified" => Some(Language::ChineseSimplified),
        "chinese_traditional" => Some(Language::ChineseTraditional),
        _ => None,
    }
}

/// What Windows' `InstallLayoutOrTip` takes to add `profile` to the current
/// user's input methods (the same text an unattended Windows setup uses):
/// the language id as four hex digits, then the class id and the profile id.
pub fn tip_spec(profile: &Profile) -> String {
    format!("{:04X}:{}{}", profile.lang_id, CLSID, profile.guid)
}

/// The languages the text service can type today. A language is added here when
/// its typing works end to end in a Windows application (composition shown,
/// text committed), not when its engine exists: Hangul first; Japanese needs the
/// candidate window too.
pub const TYPEABLE: [Language; 1] = [Language::Korean];

/// The profiles to register: the one under the home language.
pub fn registered() -> Vec<Profile> {
    profile_of(HOME).cloned().into_iter().collect()
}

/// The settings the cycle runs with: the languages the user chose (or the
/// system's, until they choose), only those in `typeable`, in their order.
///
/// If that leaves nothing because the system language is not one easime can type
/// (or everything chosen is not typeable yet), the home language is used: the
/// user picked easime in Windows' language settings to type with it. A user who
/// chose *no* language gets English only, as asked.
pub fn effective_config(
    config: &Config,
    system: &[Language],
    typeable: &[Language],
    home: Language,
) -> Config {
    let wanted = config.languages.resolved(system);
    let mut kept: Vec<Language> = wanted
        .iter()
        .copied()
        .filter(|language| typeable.contains(language))
        .collect();
    if kept.is_empty() && (config.languages.follows_system() || !wanted.is_empty()) {
        kept.push(home);
    }
    let mut config = config.clone();
    config.languages.enabled = Some(kept);
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn is_guid(text: &str) -> bool {
        let parts: Vec<&str> = text
            .strip_prefix('{')
            .and_then(|t| t.strip_suffix('}'))
            .map(|t| t.split('-').collect())
            .unwrap_or_default();
        let lengths = [8, 4, 4, 4, 12];
        parts.len() == 5
            && parts.iter().zip(lengths).all(|(part, len)| {
                part.len() == len
                    && part
                        .chars()
                        .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c))
            })
    }

    #[test]
    fn every_id_is_a_well_formed_guid_and_none_repeats() {
        let mut seen = HashSet::new();
        for guid in ALL.iter().map(|p| p.guid).chain([CLSID]) {
            assert!(is_guid(guid), "{guid}");
            assert!(seen.insert(guid), "{guid} is used twice");
        }
    }

    #[test]
    fn each_language_has_one_profile_under_its_own_windows_language() {
        let languages: HashSet<Language> = ALL.iter().map(|p| p.language).collect();
        assert_eq!(languages.len(), ALL.len(), "one profile per language");
        let ids: HashSet<u16> = ALL.iter().map(|p| p.lang_id).collect();
        assert_eq!(ids.len(), ALL.len(), "one language id per profile");
        let id = |language| ALL.iter().find(|p| p.language == language).unwrap().lang_id;
        assert_eq!(id(Language::Korean), 0x0412);
        assert_eq!(id(Language::Japanese), 0x0411);
        assert_eq!(id(Language::ChineseTraditional), 0x0404);
        assert_eq!(id(Language::ChineseSimplified), 0x0804);
    }

    #[test]
    fn the_profile_is_simply_called_easime() {
        assert!(ALL.iter().all(|profile| profile.name() == "easime"));
    }

    #[test]
    fn a_profile_is_found_by_language_and_by_the_installers_name() {
        for profile in &ALL {
            assert_eq!(profile_of(profile.language), Some(profile));
        }
        assert_eq!(profile_of(Language::English), None);
        assert_eq!(language_named("korean"), Some(Language::Korean));
        assert_eq!(language_named(" Japanese "), Some(Language::Japanese));
        assert_eq!(
            language_named("chinese_traditional"),
            Some(Language::ChineseTraditional)
        );
        assert_eq!(language_named("klingon"), None);
        assert_eq!(language_named(""), None);
    }

    #[test]
    fn the_tip_spec_is_what_windows_setup_writes() {
        let korean = profile_of(Language::Korean).unwrap();
        assert_eq!(
            tip_spec(korean),
            "0412:{CFC09673-DDBA-4C84-B57F-CF0B0DDC262C}{42FBAB7A-5B91-4A6A-868A-3CC5C8153492}"
        );
        let japanese = profile_of(Language::Japanese).unwrap();
        assert!(tip_spec(japanese).starts_with("0411:{CFC09673"));
    }

    #[test]
    fn one_profile_is_registered_under_the_home_language() {
        let registered = registered();
        assert_eq!(registered.len(), 1, "one profile, not one per language");
        assert_eq!(registered[0].language, HOME);
        assert_eq!(registered[0].lang_id, 0x0412);
        for language in TYPEABLE {
            assert!(
                easime_config::AVAILABLE_LANGUAGES.contains(&language),
                "{language:?} is typeable but has no engine"
            );
        }
    }

    fn chosen(languages: Option<Vec<Language>>) -> Config {
        let mut config = Config::default();
        config.languages.enabled = languages;
        config
    }

    fn cycle(config: &Config, system: &[Language], typeable: &[Language]) -> Vec<Language> {
        effective_config(config, system, typeable, Language::Korean)
            .languages
            .enabled
            .unwrap()
    }

    #[test]
    fn the_cycle_is_what_the_user_chose_among_what_can_be_typed() {
        use Language::*;
        let both = [Korean, Japanese];
        assert_eq!(
            cycle(&chosen(Some(vec![Japanese, Korean])), &[], &both),
            [Japanese, Korean],
            "their order"
        );
        assert_eq!(
            cycle(&chosen(Some(vec![Korean, Japanese])), &[], &[Korean]),
            [Korean],
            "what cannot be typed yet is left out"
        );
    }

    #[test]
    fn until_the_user_chooses_the_system_language_decides_and_the_home_language_backs_it_up() {
        use Language::*;
        let unchosen = chosen(None);
        assert_eq!(cycle(&unchosen, &[Korean], &[Korean]), [Korean]);
        assert_eq!(
            cycle(&unchosen, &[], &[Korean]),
            [Korean],
            "an English system: the user picked easime to type Korean with"
        );
        assert_eq!(
            cycle(&unchosen, &[Japanese], &[Korean]),
            [Korean],
            "a Japanese system, Japanese not typeable yet"
        );
    }

    #[test]
    fn choosing_no_language_means_english_only() {
        assert_eq!(
            cycle(&chosen(Some(vec![])), &[Language::Korean], &TYPEABLE),
            [],
            "an explicit choice is kept"
        );
        assert_eq!(
            cycle(&chosen(Some(vec![Language::Japanese])), &[], &TYPEABLE),
            [Language::Korean],
            "but a choice that cannot be typed yet does not leave the profile dead"
        );
    }

    #[test]
    fn nothing_but_the_language_list_changes() {
        let mut config = chosen(None);
        config.appearance.indicator_ms = 2500;
        let effective = effective_config(&config, &[], &TYPEABLE, HOME);
        assert_eq!(effective.appearance.indicator_ms, 2500);
        assert_eq!(effective.shortcuts, config.shortcuts);
    }
}
