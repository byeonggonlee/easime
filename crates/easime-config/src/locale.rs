//! Which input languages fit the language the system is set to.
//!
//! Until the user picks languages, easime enables the ones that match the
//! system language (see [`crate::Languages`]): a Korean system gets Korean, a
//! Japanese system gets Japanese, and anything else gets English only.
//!
//! Matching a language *tag* to an input language is the same everywhere
//! ([`language_for_tag`], [`languages_for_tags`]). Reading the system's
//! language is not:
//!
//! | OS | where the tags come from | state |
//! |---|---|---|
//! | Linux | `LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG` | done ([`system_languages`]) |
//! | Windows | the user's preferred UI languages (`GetUserPreferredUILanguages`) | done (the call is checked on the Windows CI runner; no Korean or Japanese Windows has run it) |
//! | macOS | the preferred languages (`AppleLanguages`, `CFLocaleCopyPreferredLanguages`) | **not done**: the environment variables are normally not set, so the result is empty, which means English only |
//!
//! An empty result is the safe fallback: nothing is switched on that the user
//! did not ask for. A macOS frontend feeds the tags of its OS to
//! [`languages_for_tags`]. See `docs/platforms.md`.

use ime_core::Language;
use std::ffi::OsString;

/// The languages that have an engine today. A language outside this list can
/// be written in the settings file but does nothing, so it is never switched on
/// for the user and the settings window does not offer it.
///
/// Keep in step with the engines the daemon builds (`frontend-wayland`).
pub const AVAILABLE_LANGUAGES: [Language; 2] = [Language::Korean, Language::Japanese];

/// The input language for one locale or language tag: `ko_KR.UTF-8`, `ja`,
/// `zh_TW`, `zh-Hant-HK`, `zh_CN@foo` and so on. `None` for languages easime
/// has no input for (and for `C` and `POSIX`).
///
/// Chinese is told apart by script or region: Traditional for `Hant` and for
/// Taiwan, Hong Kong and Macau, Simplified for `Hans` and for mainland China,
/// Singapore and Malaysia. A bare `zh` is Simplified, as in CLDR's likely
/// subtags.
pub fn language_for_tag(tag: &str) -> Option<Language> {
    let tag = tag.split(['.', '@']).next().unwrap_or("");
    let mut parts = tag.split(['_', '-']).map(str::to_ascii_lowercase);
    match parts.next()?.as_str() {
        "ko" => Some(Language::Korean),
        "ja" => Some(Language::Japanese),
        "zh" => {
            let rest: Vec<String> = parts.collect();
            let has = |part: &str| rest.iter().any(|p| p == part);
            // The script decides; the region only when there is no script.
            if has("hant") {
                Some(Language::ChineseTraditional)
            } else if has("hans") {
                Some(Language::ChineseSimplified)
            } else if has("tw") || has("hk") || has("mo") {
                Some(Language::ChineseTraditional)
            } else {
                Some(Language::ChineseSimplified)
            }
        }
        _ => None,
    }
}

/// The languages of `tags`, in order and without repeats, that easime can type
/// in today.
pub fn languages_for_tags<'a>(tags: impl IntoIterator<Item = &'a str>) -> Vec<Language> {
    let mut found = Vec::new();
    for tag in tags {
        if let Some(language) = language_for_tag(tag)
            && AVAILABLE_LANGUAGES.contains(&language)
            && !found.contains(&language)
        {
            found.push(language);
        }
    }
    found
}

/// The languages the user has told the system they use.
///
/// Windows asks the system for the preferred UI languages. Everywhere else (and
/// on Windows if that gives nothing) the environment is read like gettext does;
/// on macOS it normally says nothing, see the module documentation: `LANGUAGE` is a preference list (Plasma sets
/// it from its language settings) unless the locale is `C` or `POSIX`;
/// otherwise the first of `LC_ALL`, `LC_MESSAGES` and `LANG` that is set.
pub fn system_languages() -> Vec<Language> {
    #[cfg(windows)]
    {
        let tags = windows_api::preferred_ui_languages();
        let found = languages_for_tags(tags.iter().map(String::as_str));
        if !tags.is_empty() {
            return found;
        }
    }
    system_languages_in(&|name| std::env::var_os(name))
}

// Only Windows reads such a list; it is plain data handling, so every OS tests it.
#[cfg(any(windows, test))]
/// The names in a Windows multi-string: UTF-16 names each ended by a NUL, the
/// whole list ended by one more NUL (`"ko-KR\0en-US\0\0"`). Reading stops at
/// the end of the list, at the end of the buffer, or at an empty name.
pub fn parse_multistring(buffer: &[u16]) -> Vec<String> {
    buffer
        .split(|&unit| unit == 0)
        .take_while(|name| !name.is_empty())
        .map(String::from_utf16_lossy)
        .collect()
}

#[cfg(windows)]
mod windows_api {
    use windows_sys::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};

    /// The user's preferred UI languages as BCP 47 names (`ko-KR`, `zh-Hant-TW`),
    /// best first. Empty if the system does not say.
    pub fn preferred_ui_languages() -> Vec<String> {
        let mut count = 0u32;
        let mut length = 0u32;
        // SAFETY: with a null buffer the call only reports the size needed.
        let sized = unsafe {
            GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut count,
                std::ptr::null_mut(),
                &mut length,
            )
        };
        if sized == 0 || length == 0 {
            return Vec::new();
        }
        let mut buffer = vec![0u16; length as usize];
        // SAFETY: `buffer` holds `length` UTF-16 units, as the first call asked.
        let read = unsafe {
            GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut count,
                buffer.as_mut_ptr(),
                &mut length,
            )
        };
        if read == 0 {
            return Vec::new();
        }
        super::parse_multistring(&buffer[..(length as usize).min(buffer.len())])
    }
}

/// [`system_languages`] with the environment passed in, for tests.
pub fn system_languages_in(env: &dyn Fn(&str) -> Option<OsString>) -> Vec<Language> {
    let get = |name: &str| {
        env(name)
            .and_then(|value| value.into_string().ok())
            .filter(|value| !value.is_empty())
    };
    let locale = get("LC_ALL")
        .or_else(|| get("LC_MESSAGES"))
        .or_else(|| get("LANG"));
    let neutral = matches!(locale.as_deref(), Some("C" | "POSIX"));
    let list = if neutral { None } else { get("LANGUAGE") };
    let mut tags: Vec<&str> = match &list {
        Some(list) => list.split(':').collect(),
        None => Vec::new(),
    };
    if tags.is_empty()
        && let Some(locale) = &locale
    {
        tags.push(locale);
    }
    languages_for_tags(tags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Language::*;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
        let pairs: Vec<(String, OsString)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), OsString::from(v)))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
        }
    }

    fn system(pairs: &[(&str, &str)]) -> Vec<Language> {
        system_languages_in(&env_of(pairs))
    }

    fn utf16(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    #[test]
    fn a_windows_multistring_is_split_at_its_nuls() {
        let buffer = utf16("ko-KR\0en-US\0\0");
        assert_eq!(parse_multistring(&buffer), ["ko-KR", "en-US"]);
        // A buffer longer than the list, as the second call may leave it.
        let padded = utf16("ja-JP\0\0\0\0");
        assert_eq!(parse_multistring(&padded), ["ja-JP"]);
        // No final NUL, an empty list and an empty buffer.
        assert_eq!(parse_multistring(&utf16("zh-Hant-TW")), ["zh-Hant-TW"]);
        assert!(parse_multistring(&utf16("\0\0")).is_empty());
        assert!(parse_multistring(&[]).is_empty());
    }

    #[test]
    fn windows_language_names_map_like_any_other_tag() {
        let buffer = utf16("zh-Hant-TW\0ja-JP\0ko-KR\0en-US\0\0");
        let tags = parse_multistring(&buffer);
        assert_eq!(
            languages_for_tags(tags.iter().map(String::as_str)),
            [Japanese, Korean]
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_reports_its_preferred_languages() {
        let tags = windows_api::preferred_ui_languages();
        assert!(!tags.is_empty(), "the system has at least one UI language");
        assert!(
            tags.iter().all(|tag| tag.contains('-')),
            "BCP 47 names such as en-US: {tags:?}"
        );
        // Whatever the machine is set to, reading it must work.
        let _ = system_languages();
    }

    #[test]
    fn korean_and_japanese_tags() {
        assert_eq!(language_for_tag("ko"), Some(Korean));
        assert_eq!(language_for_tag("ko_KR.UTF-8"), Some(Korean));
        assert_eq!(language_for_tag("ko-KR"), Some(Korean));
        assert_eq!(language_for_tag("ja_JP.eucJP"), Some(Japanese));
        assert_eq!(language_for_tag("JA"), Some(Japanese));
    }

    #[test]
    fn chinese_scripts_follow_script_then_region() {
        for tag in [
            "zh_TW",
            "zh_HK.UTF-8",
            "zh_MO",
            "zh-Hant",
            "zh_Hant_CN",
            "zh-TW",
        ] {
            assert_eq!(language_for_tag(tag), Some(ChineseTraditional), "{tag}");
        }
        for tag in [
            "zh",
            "zh_CN",
            "zh_SG",
            "zh-Hans",
            "zh_Hans_TW",
            "zh_CN@pinyin",
        ] {
            assert_eq!(language_for_tag(tag), Some(ChineseSimplified), "{tag}");
        }
    }

    #[test]
    fn other_languages_and_neutral_locales_have_no_input() {
        for tag in [
            "en_US.UTF-8",
            "fr_FR",
            "de",
            "C",
            "POSIX",
            "C.UTF-8",
            "",
            "yue_HK",
        ] {
            assert_eq!(language_for_tag(tag), None, "{tag:?}");
        }
    }

    #[test]
    fn only_languages_with_an_engine_are_picked() {
        assert_eq!(
            languages_for_tags(["ja_JP", "zh_TW", "ko_KR"]),
            [Japanese, Korean]
        );
        assert_eq!(
            languages_for_tags(["zh_CN", "zh_TW"]),
            Vec::<Language>::new()
        );
        assert_eq!(languages_for_tags(["ko", "ko_KR"]), [Korean]);
        for language in AVAILABLE_LANGUAGES {
            assert_ne!(language, English);
        }
    }

    #[test]
    fn a_korean_or_japanese_system_enables_its_language() {
        assert_eq!(system(&[("LANG", "ko_KR.UTF-8")]), [Korean]);
        assert_eq!(system(&[("LANG", "ja_JP.UTF-8")]), [Japanese]);
        assert_eq!(
            system(&[("LC_ALL", "ko_KR.UTF-8"), ("LANG", "en_US.UTF-8")]),
            [Korean]
        );
        assert_eq!(
            system(&[("LC_MESSAGES", "ja_JP.UTF-8"), ("LANG", "en_US")]),
            [Japanese]
        );
    }

    #[test]
    fn any_other_system_enables_nothing() {
        assert!(system(&[("LANG", "en_US.UTF-8")]).is_empty());
        assert!(system(&[("LANG", "fr_FR.UTF-8")]).is_empty());
        assert!(system(&[("LANG", "C.UTF-8")]).is_empty());
        assert!(
            system(&[("LANG", "zh_CN.UTF-8")]).is_empty(),
            "no Chinese engine yet"
        );
        assert!(system(&[]).is_empty());
    }

    #[test]
    fn the_language_list_counts_in_order_and_keeps_each_language_once() {
        assert_eq!(system(&[("LANGUAGE", "ja:ko:en")]), [Japanese, Korean]);
        assert_eq!(
            system(&[("LANGUAGE", "en_US:ko"), ("LANG", "en_US.UTF-8")]),
            [Korean]
        );
        assert_eq!(system(&[("LANGUAGE", "ko_KR:ko:ja")]), [Korean, Japanese]);
        assert_eq!(
            system(&[("LANGUAGE", "ko"), ("LANG", "ja_JP.UTF-8")]),
            [Korean]
        );
    }

    #[test]
    fn the_language_list_is_ignored_in_the_c_locale() {
        assert!(system(&[("LANGUAGE", "ko"), ("LANG", "C")]).is_empty());
        assert!(system(&[("LANGUAGE", "ko"), ("LC_ALL", "POSIX")]).is_empty());
        // An empty value counts as not set.
        assert_eq!(system(&[("LANGUAGE", ""), ("LANG", "ko_KR")]), [Korean]);
    }
}
