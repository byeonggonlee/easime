//! The values the QML pages bind to, as plain strings and numbers. Words the
//! user reads are written in QML (so they can be translated); this only
//! produces codes and technical details.

use crate::model::{Failure, FailureKind, Model, backspace_code, language_code, toggle_key_code};
use std::path::PathBuf;

/// The code QML switches on for a failure.
pub fn failure_code(kind: FailureKind) -> &'static str {
    match kind {
        FailureKind::Unreadable => "unreadable",
        FailureKind::NoHome => "no_home",
        FailureKind::WriteFailed => "write_failed",
        FailureKind::Invalid => "invalid",
    }
}

/// Everything the pages show about the current settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    /// Enabled languages in switching order, comma separated (`korean,japanese`).
    pub languages: String,
    /// True while the user has not chosen languages and the system language
    /// decides which are on.
    pub languages_follow_system: bool,
    pub toggle_key: String,
    pub backspace: String,
    pub indicator_box: bool,
    pub indicator_ms: i32,
    /// Empty when the settings were read fine.
    pub problem_kind: String,
    pub problem_detail: String,
}

impl View {
    pub fn of(model: &Model) -> Self {
        let config = model.config();
        let languages: Vec<&str> = model.languages().into_iter().map(language_code).collect();
        let (problem_kind, problem_detail) = match model.problem() {
            Some(Failure { kind, detail }) => (failure_code(*kind).to_owned(), detail.clone()),
            None => (String::new(), String::new()),
        };
        Self {
            languages: languages.join(","),
            languages_follow_system: model.follows_system_language(),
            toggle_key: toggle_key_code(config.shortcuts.toggle_key).to_owned(),
            backspace: backspace_code(config.korean.backspace).to_owned(),
            indicator_box: config.appearance.indicator_box,
            indicator_ms: i32::try_from(config.appearance.indicator_ms).unwrap_or(i32::MAX),
            problem_kind,
            problem_detail,
        }
    }
}

/// Where the daemon writes its log: `$XDG_STATE_HOME/easime/easime.log`,
/// else under `~/.local/state`. Only absolute directories count.
pub fn log_file(env: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    let absolute = |name: &str| env(name).map(PathBuf::from).filter(|p| p.is_absolute());
    let state = absolute("XDG_STATE_HOME")
        .or_else(|| absolute("HOME").map(|home| home.join(".local").join("state")))?;
    Some(state.join("easime").join("easime.log"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ime_core::Language;
    use std::ffi::OsString;

    #[test]
    fn the_default_view_matches_the_default_settings() {
        let view = View::of(&Model::open_with_system(
            Some(
                std::env::temp_dir()
                    .join("easime-view-test-missing")
                    .join("c.toml"),
            ),
            vec![Language::Korean],
        ));
        assert_eq!(view.languages, "korean");
        assert!(view.languages_follow_system);
        assert_eq!(view.toggle_key, "both");
        assert_eq!(view.backspace, "jamo");
        assert!(view.indicator_box);
        assert_eq!(view.indicator_ms, 1000);
        assert_eq!(view.problem_kind, "");
    }

    #[test]
    fn a_missing_home_shows_as_a_problem() {
        let view = View::of(&Model::open_with_system(None, Vec::new()));
        assert_eq!(view.problem_kind, "no_home");
        assert_eq!(view.languages, "");
    }

    #[test]
    fn failure_codes_are_distinct() {
        let codes = [
            FailureKind::Unreadable,
            FailureKind::NoHome,
            FailureKind::WriteFailed,
            FailureKind::Invalid,
        ]
        .map(failure_code);
        for (i, a) in codes.iter().enumerate() {
            for b in &codes[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

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

    /// An absolute path on the OS running the tests.
    fn abs(path: &str) -> String {
        std::env::temp_dir()
            .join(path)
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn the_log_follows_the_state_directory_rules() {
        let state = abs("state");
        let home = abs("home");
        let from_state = log_file(env_of(&[("XDG_STATE_HOME", &state), ("HOME", &home)]));
        assert_eq!(
            from_state,
            Some(PathBuf::from(&state).join("easime").join("easime.log"))
        );
        let from_home = log_file(env_of(&[("HOME", &home)]));
        assert_eq!(
            from_home,
            Some(
                PathBuf::from(&home)
                    .join(".local")
                    .join("state")
                    .join("easime")
                    .join("easime.log")
            )
        );
        // A relative XDG_STATE_HOME is ignored.
        let relative = log_file(env_of(&[("XDG_STATE_HOME", "state"), ("HOME", &home)]));
        assert_eq!(relative, from_home);
        assert_eq!(log_file(env_of(&[])), None);
    }
}
