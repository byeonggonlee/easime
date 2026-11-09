//! What easime registers with Windows: one text service with **one profile**,
//! listed under **Korean** (decision of 2026-11-08, `docs/decisions.md`; it
//! replaced "one profile per language" the same day).
//!
//! Inside the one profile easime cycles through the languages the user chose
//! with the one switch key, English, Korean, Japanese, English, ... like the
//! Linux version, so the cycle and the settings are the same everywhere. The
//! Windows language indicator keeps showing Korean whichever one is typed; the
//! box next to the cursor says which one is on.
//!
//! **Not for every language.** The user decided to try registering it for every
//! language (`RegisterProfile` with `0xFFFF`) so it could be switched on under
//! a language they already have. The CI showed such a profile listed under
//! every language, but also started in programs where nobody chose it (the
//! user's other programs had it running right after it was switched on), and
//! uninstalling then had Windows close those programs. So it is listed under
//! Korean again (`docs/records/2026-11-08-windows-plan.md`, step 2).
//!
//! Only languages the text service can really type are in the cycle
//! ([`TYPEABLE`]), so Windows never offers an input method that does nothing
//! (DESIGN.md 3.1). The GUIDs are fixed for ever: Windows remembers the user's
//! choice by them.

use easime_config::Config;
use ime_core::Language;

/// The class id of the easime text service.
pub const CLSID: &str = "{CFC09673-DDBA-4C84-B57F-CF0B0DDC262C}";

/// Korean (ko-KR), the language easime is listed under.
pub const KOREAN: u16 = 0x0412;

/// One input method in Windows' language list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
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

/// The profile easime registers: one, under Korean.
pub const REGISTERED: Profile = Profile {
    lang_id: KOREAN,
    guid: "{05C63458-0689-4061-8D1B-D8C77D6A613C}",
};

/// Profiles earlier test builds registered (one per language, then the Korean
/// one with another id). Registering and removing take them away too, so an
/// upgrade leaves only [`REGISTERED`].
pub const EARLIER: [Profile; 4] = [
    Profile {
        lang_id: KOREAN,
        guid: "{42FBAB7A-5B91-4A6A-868A-3CC5C8153492}",
    },
    Profile {
        lang_id: 0x0411, // ja-JP
        guid: "{C059F0B3-8110-4CA5-AF4F-B2C79C3BE921}",
    },
    Profile {
        lang_id: 0x0404, // zh-TW
        guid: "{25E11A69-7BDC-4491-B31D-3060F63CA7D4}",
    },
    Profile {
        lang_id: 0x0804, // zh-CN
        guid: "{F2EC3B81-9A21-4B5C-BD2D-D6D776A9DC72}",
    },
];

/// What Windows' `InstallLayoutOrTip` takes to switch `profile` on for the
/// current user under the language `lang_id` (the same text an unattended
/// Windows setup uses): the language as four hex digits, then the class id and
/// the profile id.
pub fn tip_spec(lang_id: u16, profile: &Profile) -> String {
    format!("{lang_id:04X}:{CLSID}{}", profile.guid)
}

/// Whether an input method entry of the user's language settings (the value
/// names under `Control Panel\International\User Profile\<language>`) is
/// easime's, from this build or an earlier one.
pub fn is_ours(entry: &str) -> bool {
    entry.to_ascii_uppercase().contains(CLSID)
}

/// The language the cycle falls back to (see [`effective_config`]).
pub const HOME: Language = Language::Korean;

/// The languages the text service can type today. A language is added here when
/// its typing works end to end in a Windows application (composition shown,
/// text committed, candidates chosen in the candidate window), not when its
/// engine exists: Korean, then Japanese once the candidate window was there.
pub const TYPEABLE: [Language; 2] = [Language::Korean, Language::Japanese];

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
        let profiles = EARLIER.iter().chain([&REGISTERED]);
        for guid in profiles.map(|p| p.guid).chain([CLSID]) {
            assert!(is_guid(guid), "{guid}");
            assert!(seen.insert(guid), "{guid} is used twice");
        }
    }

    #[test]
    fn korean_and_japanese_can_be_typed() {
        assert_eq!(TYPEABLE, [Language::Korean, Language::Japanese]);
        assert_eq!(
            cycle(
                &chosen(Some(vec![Language::Japanese, Language::Korean])),
                &[],
                &TYPEABLE
            ),
            [Language::Japanese, Language::Korean],
            "in the order the user chose"
        );
    }

    #[test]
    fn one_profile_is_registered_under_korean() {
        assert_eq!(REGISTERED.lang_id, 0x0412);
        assert_eq!(REGISTERED.name(), "easime");
        assert!(!EARLIER.contains(&REGISTERED));
        for language in TYPEABLE {
            assert!(
                easime_config::AVAILABLE_LANGUAGES.contains(&language),
                "{language:?} is typeable but has no engine"
            );
        }
    }

    #[test]
    fn the_tip_spec_is_what_windows_setup_writes() {
        assert_eq!(
            tip_spec(KOREAN, &REGISTERED),
            "0412:{CFC09673-DDBA-4C84-B57F-CF0B0DDC262C}{05C63458-0689-4061-8D1B-D8C77D6A613C}"
        );
        assert!(tip_spec(0x0412, &EARLIER[0]).starts_with("0412:{CFC09673"));
    }

    #[test]
    fn our_entries_in_the_users_settings_are_recognised() {
        assert!(is_ours(&tip_spec(KOREAN, &REGISTERED)));
        assert!(is_ours(&tip_spec(0x0412, &EARLIER[0]).to_ascii_lowercase()));
        assert!(!is_ours(
            "0412:{A028AE76-01B1-46C2-99C4-ACD9858AE02F}{B5FE1F02-D5F2-4445-9C03-C568F23C99A1}"
        ));
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
            cycle(
                &chosen(Some(vec![Language::ChineseSimplified])),
                &[],
                &TYPEABLE
            ),
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
