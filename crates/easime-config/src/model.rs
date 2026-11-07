//! The settings data model.

use ime_core::Language;
use serde::{Deserialize, Serialize};

/// Which key switches between English and the current language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToggleKey {
    /// The dedicated Hangul key.
    Hangul,
    /// Right Alt, when tapped on its own.
    RightAlt,
    /// Either of the two.
    #[default]
    Both,
}

impl ToggleKey {
    pub fn uses_hangul_key(self) -> bool {
        matches!(self, ToggleKey::Hangul | ToggleKey::Both)
    }

    pub fn uses_right_alt(self) -> bool {
        matches!(self, ToggleKey::RightAlt | ToggleKey::Both)
    }
}

/// What Backspace removes while a Hangul syllable is being composed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackspaceUnit {
    /// One jamo at a time: 한 → 하 → ㅎ.
    #[default]
    Jamo,
    /// The whole syllable.
    Syllable,
}

/// Which languages the toggle key cycles through, after English.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Languages {
    /// The languages the user chose, in switching order. `None` while the user
    /// has not chosen: the languages that match the system language are used
    /// then (see [`Languages::resolved`]). `Some(vec![])` is a choice too:
    /// English only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<Vec<Language>>,
}

impl Languages {
    /// The languages to cycle through: the user's choice, or else the
    /// `system` languages (what [`crate::system_languages`] returns).
    pub fn resolved(&self, system: &[Language]) -> Vec<Language> {
        match &self.enabled {
            Some(chosen) => chosen.clone(),
            None => system.to_vec(),
        }
    }

    /// True while the user has not chosen and the system language decides.
    pub fn follows_system(&self) -> bool {
        self.enabled.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Shortcuts {
    pub toggle_key: ToggleKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Korean {
    pub backspace: BackspaceUnit,
}

/// How easime looks on screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Show a small box with the language glyph next to the text cursor when
    /// the user switches language. Focusing a text field does not show it.
    pub indicator_box: bool,
    /// How long the box stays visible, in milliseconds.
    pub indicator_ms: u32,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            indicator_box: true,
            indicator_ms: 1000,
        }
    }
}

/// All settings.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub languages: Languages,
    pub shortcuts: Shortcuts,
    pub korean: Korean,
    pub appearance: Appearance,
}

impl Config {
    /// Parse a TOML document. Missing fields take their defaults.
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("config always serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_documented_decisions() {
        let c = Config::default();
        assert_eq!(c.shortcuts.toggle_key, ToggleKey::Both);
        assert_eq!(c.korean.backspace, BackspaceUnit::Jamo);
        assert!(c.appearance.indicator_box);
        assert_eq!(c.appearance.indicator_ms, 1000);
    }

    #[test]
    fn appearance_can_be_set_and_keeps_other_defaults() {
        let c = Config::from_toml("[appearance]\nindicator_box = false\n").unwrap();
        assert!(!c.appearance.indicator_box);
        assert_eq!(c.appearance.indicator_ms, 1000);
        let c = Config::from_toml("[appearance]\nindicator_ms = 250\n").unwrap();
        assert!(c.appearance.indicator_box);
        assert_eq!(c.appearance.indicator_ms, 250);
    }

    #[test]
    fn negative_indicator_time_is_an_error() {
        assert!(Config::from_toml("[appearance]\nindicator_ms = -5\n").is_err());
    }

    #[test]
    fn languages_follow_the_system_until_chosen() {
        let c = Config::default();
        assert!(c.languages.follows_system());
        assert_eq!(c.languages.resolved(&[]), vec![]);
        assert_eq!(
            c.languages.resolved(&[Language::Korean]),
            vec![Language::Korean]
        );
        // Neither an empty file nor a file without the key is a choice.
        assert!(Config::from_toml("").unwrap().languages.follows_system());
        let c = Config::from_toml("[languages]\n").unwrap();
        assert!(c.languages.follows_system());
        let c = Config::from_toml("[korean]\nbackspace = \"syllable\"\n").unwrap();
        assert!(c.languages.follows_system());
    }

    #[test]
    fn a_chosen_list_wins_over_the_system_even_when_empty() {
        let c = Config::from_toml("[languages]\nenabled = [\"japanese\"]\n").unwrap();
        assert!(!c.languages.follows_system());
        assert_eq!(
            c.languages.resolved(&[Language::Korean]),
            vec![Language::Japanese]
        );
        let none = Config::from_toml("[languages]\nenabled = []\n").unwrap();
        assert!(!none.languages.follows_system());
        assert_eq!(none.languages.resolved(&[Language::Korean]), vec![]);
    }

    #[test]
    fn the_default_file_does_not_make_a_choice() {
        assert!(!Config::default().to_toml().contains("enabled"));
    }

    #[test]
    fn languages_are_read_in_order() {
        let c = Config::from_toml("[languages]\nenabled = [\"japanese\", \"korean\"]\n").unwrap();
        assert_eq!(
            c.languages.enabled,
            Some(vec![Language::Japanese, Language::Korean])
        );
    }

    #[test]
    fn the_chinese_scripts_are_separate_languages() {
        let c = Config::from_toml(
            "[languages]\nenabled = [\"chinese_traditional\", \"chinese_simplified\"]\n",
        )
        .unwrap();
        assert_eq!(
            c.languages.enabled,
            Some(vec![
                Language::ChineseTraditional,
                Language::ChineseSimplified
            ])
        );
        assert!(Config::from_toml("[languages]\nenabled = [\"chinese\"]\n").is_err());
    }

    #[test]
    fn unknown_language_is_an_error() {
        assert!(Config::from_toml("[languages]\nenabled = [\"klingon\"]\n").is_err());
    }

    #[test]
    fn empty_file_gives_defaults() {
        assert_eq!(Config::from_toml("").unwrap(), Config::default());
    }

    #[test]
    fn partial_file_keeps_other_defaults() {
        let c = Config::from_toml("[korean]\nbackspace = \"syllable\"\n").unwrap();
        assert_eq!(c.korean.backspace, BackspaceUnit::Syllable);
        assert_eq!(c.shortcuts.toggle_key, ToggleKey::Both);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let text = "future_option = 1\n[shortcuts]\ntoggle_key = \"right_alt\"\nnew = true\n";
        let c = Config::from_toml(text).unwrap();
        assert_eq!(c.shortcuts.toggle_key, ToggleKey::RightAlt);
    }

    #[test]
    fn invalid_value_is_an_error() {
        assert!(Config::from_toml("[shortcuts]\ntoggle_key = \"capslock\"\n").is_err());
    }

    #[test]
    fn round_trips() {
        let c = Config {
            languages: Languages {
                enabled: Some(vec![Language::Korean, Language::Japanese]),
            },
            shortcuts: Shortcuts {
                toggle_key: ToggleKey::Hangul,
            },
            korean: Korean {
                backspace: BackspaceUnit::Syllable,
            },
            appearance: Appearance {
                indicator_box: false,
                indicator_ms: 400,
            },
        };
        assert_eq!(Config::from_toml(&c.to_toml()).unwrap(), c);
    }

    #[test]
    fn toggle_key_helpers() {
        assert!(ToggleKey::Both.uses_hangul_key() && ToggleKey::Both.uses_right_alt());
        assert!(ToggleKey::Hangul.uses_hangul_key() && !ToggleKey::Hangul.uses_right_alt());
        assert!(!ToggleKey::RightAlt.uses_hangul_key() && ToggleKey::RightAlt.uses_right_alt());
    }
}
