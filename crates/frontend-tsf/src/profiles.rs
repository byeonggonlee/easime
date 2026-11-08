//! What easime registers with Windows: one text service, with one profile per
//! language (decision of 2026-11-08, `docs/decisions.md`).
//!
//! Windows lists input methods per language, and a profile belongs to exactly
//! one. So the user picks the language with the OS (Win+Space, the language
//! indicator of the task bar) and easime only switches between English and that
//! language inside it: a profile is a `ime_session::Session` for one language
//! ([`ime_session::Session::for_language`]).
//!
//! Only languages that this frontend can really type get a profile, so Windows
//! never offers an input method that does nothing (DESIGN.md 3.1): the language
//! needs an engine ([`easime_config::AVAILABLE_LANGUAGES`]) and the text
//! service must be able to drive it ([`TYPEABLE`]). The GUIDs are fixed for
//! ever: Windows remembers the user's choice by them.

use ime_core::Language;
use ime_session::indicator::language_native_name;

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
    /// The name Windows shows for it: easime, then the language in its own
    /// script (`easime (한국어)`).
    pub fn name(&self) -> String {
        format!("easime ({})", language_native_name(self.language))
    }
}

/// Every profile easime will have, whether or not its engine exists yet.
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

/// The profiles to register: those whose language has an engine and that the
/// text service can type.
pub fn registered() -> Vec<Profile> {
    ALL.iter()
        .filter(|profile| {
            easime_config::AVAILABLE_LANGUAGES.contains(&profile.language)
                && TYPEABLE.contains(&profile.language)
        })
        .cloned()
        .collect()
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
    fn profiles_are_named_after_their_language() {
        let names: Vec<String> = ALL.iter().map(Profile::name).collect();
        assert_eq!(
            names,
            [
                "easime (한국어)",
                "easime (日本語)",
                "easime (繁體中文)",
                "easime (简体中文)"
            ]
        );
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
    fn only_languages_with_an_engine_that_can_be_typed_are_registered() {
        let languages: Vec<Language> = registered().iter().map(|p| p.language).collect();
        assert_eq!(languages, [Language::Korean]);
        for language in TYPEABLE {
            assert!(
                easime_config::AVAILABLE_LANGUAGES.contains(&language),
                "{language:?} is typeable but has no engine"
            );
        }
        assert!(
            !languages.contains(&Language::Japanese),
            "Japanese needs the candidate window first"
        );
        assert!(
            !languages.contains(&Language::ChineseSimplified),
            "no Chinese engine yet, so no Chinese profile"
        );
    }
}
