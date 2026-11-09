//! The words of the settings app in the user's language. The table is the
//! KDE window's (`settings/kde/qml/tr.js`, Korean source text as the key, read
//! here when the program is built), so both apps say the same thing and a
//! translation is fixed once. The language follows the same rule: what was
//! asked for (`--ui-language=`), else the first of the system's languages that
//! has a translation, else English.
//!
//! The English, Japanese and Chinese texts are drafts that no native speaker
//! has reviewed (DESIGN.md 4.1): [`Words::is_draft`] lets the app say so.

use std::collections::HashMap;

const SOURCE: &str = include_str!("../../kde/qml/tr.js");

type Table = HashMap<String, HashMap<String, String>>;

/// The table between the BEGIN and END lines of `tr.js` (plain JSON).
fn parse(source: &str) -> Result<Table, String> {
    let begin = source.find("// BEGIN TABLE").ok_or("no BEGIN TABLE line")?;
    let end = source.find("// END TABLE").ok_or("no END TABLE line")?;
    let body = source.get(begin..end).ok_or("END before BEGIN")?;
    let json = body
        .get(body.find('{').ok_or("no table")?..)
        .unwrap_or_default();
    serde_json::from_str(json.trim_end().trim_end_matches(';')).map_err(|e| e.to_string())
}

/// One of [`LANGUAGES`] for a tag such as `ja-JP`, `zh_TW` or `zh-Hant-HK`.
pub fn pick(tag: &str) -> Option<&'static str> {
    let tag = tag.replace('_', "-").to_lowercase();
    let is = |prefix: &str| tag == prefix || tag.starts_with(&format!("{prefix}-"));
    if is("ko") {
        Some("ko")
    } else if is("ja") {
        Some("ja")
    } else if is("en") {
        Some("en")
    } else if is("zh") {
        // Traditional characters: the Hant script, or Taiwan, Hong Kong, Macau.
        let traditional = tag.contains("-hant")
            || ["-tw", "-hk", "-mo"]
                .iter()
                .any(|region| tag.ends_with(region) || tag.contains(&format!("{region}-")));
        Some(if traditional { "zh-Hant" } else { "zh-Hans" })
    } else {
        None
    }
}

/// The display language: `requested` if it is one, else the first of the
/// system's languages that is, else English.
pub fn detect(requested: Option<&str>, system: &[String]) -> &'static str {
    requested
        .and_then(pick)
        .or_else(|| system.iter().find_map(|tag| pick(tag)))
        .unwrap_or("en")
}

/// The language asked for on the command line (`--ui-language=ja`).
pub fn requested(args: impl IntoIterator<Item = String>) -> Option<String> {
    args.into_iter()
        .find_map(|arg| arg.strip_prefix("--ui-language=").map(str::to_owned))
}

/// The words in one language.
pub struct Words {
    language: &'static str,
    table: Table,
}

impl Words {
    pub fn new(language: &'static str) -> Self {
        // The table is checked by the tests, so it always parses; an empty one
        // would still show the Korean source text.
        let table = parse(SOURCE).unwrap_or_default();
        Self { language, table }
    }

    /// The translations other than Korean are unreviewed drafts.
    pub fn is_draft(&self) -> bool {
        self.language != "ko"
    }

    /// `source` (the Korean text) in this language; the source itself when
    /// the table has no translation.
    pub fn t(&self, source: &str) -> String {
        if self.language == "ko" {
            return source.to_owned();
        }
        self.table
            .get(source)
            .and_then(|row| row.get(self.language))
            .filter(|text| !text.is_empty())
            .cloned()
            .unwrap_or_else(|| source.to_owned())
    }

    /// [`Words::t`] with `%1` replaced.
    pub fn t1(&self, source: &str, argument: &str) -> String {
        self.t(source).replace("%1", argument)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The display languages, Korean (the source) first.
    const LANGUAGES: [&str; 5] = ["ko", "en", "ja", "zh-Hant", "zh-Hans"];

    #[test]
    fn the_shared_table_parses_and_every_text_has_every_language() {
        let table = parse(SOURCE).expect("tr.js holds a JSON table");
        assert!(table.len() > 50, "{} texts", table.len());
        for (source, row) in &table {
            for language in &LANGUAGES[1..] {
                assert!(
                    row.get(*language).is_some_and(|text| !text.is_empty()),
                    "{language} lacks {source:?}"
                );
            }
        }
    }

    /// The Korean string literals of the window's code, unescaped.
    fn korean_literals(code: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut rest = code;
        while let Some(start) = rest.find('"') {
            let after = &rest[start + 1..];
            let mut text = String::new();
            let mut chars = after.char_indices();
            let mut end = after.len();
            while let Some((i, c)) = chars.next() {
                match c {
                    '\\' => match chars.next() {
                        Some((_, 'n')) => text.push('\n'),
                        Some((_, other)) => text.push(other),
                        None => {}
                    },
                    '"' => {
                        end = i;
                        break;
                    }
                    c => text.push(c),
                }
            }
            if text.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c)) {
                found.push(text);
            }
            rest = &after[(end + 1).min(after.len())..];
        }
        found
    }

    #[test]
    fn every_korean_text_of_the_window_is_in_the_shared_table() {
        let table = parse(SOURCE).unwrap();
        let texts = korean_literals(include_str!("app.rs"));
        assert!(texts.len() > 30, "{} texts", texts.len());
        let missing: Vec<&String> = texts
            .iter()
            .filter(|text| !table.contains_key(*text))
            .collect();
        assert!(
            missing.is_empty(),
            "not in settings/kde/qml/tr.js: {missing:#?}"
        );
    }

    #[test]
    fn a_language_is_picked_from_a_tag() {
        assert_eq!(pick("ko-KR"), Some("ko"));
        assert_eq!(pick("ja"), Some("ja"));
        assert_eq!(pick("en-GB"), Some("en"));
        assert_eq!(pick("zh-TW"), Some("zh-Hant"));
        assert_eq!(pick("zh_HK"), Some("zh-Hant"));
        assert_eq!(pick("zh-Hant-MO"), Some("zh-Hant"));
        assert_eq!(pick("zh-CN"), Some("zh-Hans"));
        assert_eq!(pick("zh-Hans-SG"), Some("zh-Hans"));
        assert_eq!(pick("de-DE"), None);
        assert_eq!(pick("kok"), None, "Konkani is not Korean");
    }

    #[test]
    fn the_request_wins_then_the_system_then_english() {
        let system = ["de-DE".to_owned(), "ja-JP".to_owned()];
        assert_eq!(detect(Some("zh-TW"), &system), "zh-Hant");
        assert_eq!(detect(Some("xx"), &system), "ja");
        assert_eq!(detect(None, &system), "ja");
        assert_eq!(detect(None, &["fr-FR".to_owned()]), "en");
        let args = ["easime-settings", "--ui-language=ko"].map(String::from);
        assert_eq!(requested(args).as_deref(), Some("ko"));
    }

    #[test]
    fn words_are_translated_and_fall_back_to_the_source() {
        let japanese = Words::new("ja");
        assert_eq!(japanese.t("일반"), "全般");
        assert!(japanese.is_draft());
        assert_eq!(japanese.t("표에 없는 말"), "표에 없는 말");
        let korean = Words::new("ko");
        assert_eq!(korean.t("일반"), "일반");
        assert!(!korean.is_draft());
        assert_eq!(
            Words::new("en").t1("전환 순서: %1", "English → 한국어"),
            "Switching order: English → 한국어"
        );
    }
}
