//! What easime registers with Windows: one text service, with one profile per
//! language (decision of 2026-11-08, `docs/decisions.md`).
//!
//! Windows lists input methods per language, and a profile belongs to exactly
//! one. So the user picks the language with the OS (Win+Space, the language
//! indicator of the task bar) and easime only switches between English and that
//! language inside it: a profile is a `ime_session::Session` for one language
//! ([`ime_session::Session::for_language`]).
//!
//! Only languages that have an engine get a profile, so Windows never offers an
//! input method that does nothing (DESIGN.md 3.1). The GUIDs are fixed for
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

/// The profiles to register: those whose language has an engine.
pub fn registered() -> Vec<Profile> {
    ALL.iter()
        .filter(|profile| easime_config::AVAILABLE_LANGUAGES.contains(&profile.language))
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
    fn only_languages_with_an_engine_are_registered() {
        let languages: Vec<Language> = registered().iter().map(|p| p.language).collect();
        assert_eq!(languages, easime_config::AVAILABLE_LANGUAGES);
        assert!(
            !languages.contains(&Language::ChineseSimplified),
            "no Chinese engine yet, so no Chinese profile"
        );
    }
}
