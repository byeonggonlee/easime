//! The values the QML pages bind to, as plain strings and numbers. Words the
//! user reads are written in QML (so they can be translated); this only
//! produces codes and technical details.

use crate::activation::{Activation, State};
use crate::model::{Failure, FailureKind, Model, backspace_code, language_code, toggle_key_code};

/// The code QML switches on for a failure.
pub fn failure_code(kind: FailureKind) -> &'static str {
    match kind {
        FailureKind::Unreadable => "unreadable",
        FailureKind::NoHome => "no_home",
        FailureKind::WriteFailed => "write_failed",
        FailureKind::Invalid => "invalid",
        FailureKind::ActivationChanged => "activation_changed",
    }
}

/// What the page that offers to select easime as the input method shows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActivationView {
    /// `unsupported`, `not_installed`, `other`, `selected` (chosen, starts at
    /// the next login) or `running`.
    pub state: String,
    /// The input method selected now, by name; empty if none.
    pub current: String,
    /// Whether the change can be undone, and to what (empty: "none").
    pub can_undo: bool,
    pub undo_name: String,
    /// The file the change goes to.
    pub file: String,
}

impl ActivationView {
    pub fn of(activation: &Activation) -> Self {
        let mut view = Self {
            file: activation
                .kwinrc()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            ..Self::default()
        };
        match activation.state() {
            State::Unsupported => view.state = "unsupported".into(),
            State::NotInstalled => view.state = "not_installed".into(),
            State::Other { current } => {
                view.state = "other".into();
                view.current = current.unwrap_or_default();
            }
            State::Selected { running, can_undo } => {
                view.state = if running { "running" } else { "selected" }.into();
                view.can_undo = can_undo;
                view.undo_name = activation.undo_target().flatten().unwrap_or_default();
            }
        }
        view
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

#[cfg(test)]
mod tests {
    use super::*;
    use ime_core::Language;

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

    fn activation_in(dir: &std::path::Path, kwinrc: Option<&str>, running: bool) -> Activation {
        let desktop = dir.join("easime.desktop");
        std::fs::write(&desktop, "").unwrap();
        if let Some(text) = kwinrc {
            std::fs::write(dir.join("kwinrc"), text).unwrap();
        }
        Activation::new(
            true,
            Some(dir.join("kwinrc")),
            Some(desktop),
            Some(dir.join("backup.toml")),
            Box::new(move || running),
        )
    }

    #[test]
    fn the_activation_page_shows_each_state() {
        let dir =
            std::env::temp_dir().join(format!("easime-view-activation-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let other = activation_in(
            &dir,
            Some("[Wayland]\nInputMethod=/x/org.kde.plasma.keyboard.desktop\n"),
            false,
        );
        let view = ActivationView::of(&other);
        assert_eq!(
            (view.state.as_str(), view.current.as_str()),
            ("other", "org.kde.plasma.keyboard")
        );
        assert!(!view.can_undo);
        assert_eq!(view.file, dir.join("kwinrc").to_string_lossy());

        other.activate().unwrap();
        let view = ActivationView::of(&other);
        assert_eq!(view.state, "selected");
        assert!(view.can_undo);
        assert_eq!(view.undo_name, "org.kde.plasma.keyboard");

        let running = activation_in(&dir, None, true);
        assert_eq!(ActivationView::of(&running).state, "running");

        let mut not_kde = activation_in(&dir, None, false);
        not_kde.set_kde_for_test(false);
        assert_eq!(ActivationView::of(&not_kde).state, "unsupported");
        std::fs::remove_dir_all(&dir).ok();
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
            FailureKind::ActivationChanged,
        ]
        .map(failure_code);
        for (i, a) in codes.iter().enumerate() {
            for b in &codes[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
