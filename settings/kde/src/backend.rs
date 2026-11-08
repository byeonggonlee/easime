//! The QObject the QML pages talk to. It owns the [`Model`], turns the
//! model's state into properties (so bindings update), and reports what
//! happened after each action in `resultKind` / `resultDetail`.

use crate::activation::{Activation, Failure as ActivationFailure};
use crate::model::{
    Failure, FailureKind, Model, backspace_from_code, language_from_code, toggle_key_from_code,
};
use crate::view::{ActivationView, View, failure_code};
use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(QString, activation_state, cxx_name = "activationState")]
        #[qproperty(QString, activation_current, cxx_name = "activationCurrent")]
        #[qproperty(bool, activation_can_undo, cxx_name = "activationCanUndo")]
        #[qproperty(QString, activation_undo_name, cxx_name = "activationUndoName")]
        #[qproperty(QString, activation_file, cxx_name = "activationFile")]
        #[qproperty(QString, languages)]
        #[qproperty(bool, languages_follow_system, cxx_name = "languagesFollowSystem")]
        #[qproperty(QString, toggle_key, cxx_name = "toggleKey")]
        #[qproperty(QString, backspace)]
        #[qproperty(bool, indicator_box, cxx_name = "indicatorBox")]
        #[qproperty(i32, indicator_ms, cxx_name = "indicatorMs")]
        #[qproperty(QString, problem_kind, cxx_name = "problemKind")]
        #[qproperty(QString, problem_detail, cxx_name = "problemDetail")]
        #[qproperty(QString, result_kind, cxx_name = "resultKind")]
        #[qproperty(QString, result_detail, cxx_name = "resultDetail")]
        #[qproperty(QString, dictionary_state, cxx_name = "dictionaryState")]
        #[qproperty(QString, dictionary_detail, cxx_name = "dictionaryDetail")]
        #[qproperty(QString, config_path, cxx_name = "configPath")]
        #[qproperty(QString, log_path, cxx_name = "logPath")]
        type EasimeSettings = super::EasimeSettingsRust;

        #[qinvokable]
        #[cxx_name = "setLanguageEnabled"]
        fn set_language_enabled(self: Pin<&mut EasimeSettings>, code: &QString, enabled: bool);

        #[qinvokable]
        #[cxx_name = "moveLanguage"]
        fn move_language(self: Pin<&mut EasimeSettings>, code: &QString, up: bool);

        #[qinvokable]
        #[cxx_name = "applyToggleKey"]
        fn set_toggle_key_code(self: Pin<&mut EasimeSettings>, code: &QString);

        #[qinvokable]
        #[cxx_name = "applyBackspace"]
        fn set_backspace_code(self: Pin<&mut EasimeSettings>, code: &QString);

        #[qinvokable]
        #[cxx_name = "applyIndicatorBox"]
        fn set_indicator_box_shown(self: Pin<&mut EasimeSettings>, shown: bool);

        #[qinvokable]
        #[cxx_name = "applyIndicatorMs"]
        fn set_indicator_milliseconds(self: Pin<&mut EasimeSettings>, milliseconds: i32);

        #[qinvokable]
        #[cxx_name = "applyActivation"]
        fn apply_activation(self: Pin<&mut EasimeSettings>);

        #[qinvokable]
        #[cxx_name = "undoActivation"]
        fn undo_activation(self: Pin<&mut EasimeSettings>);

        #[qinvokable]
        #[cxx_name = "clearResult"]
        fn clear_result(self: Pin<&mut EasimeSettings>);

        #[qinvokable]
        fn reload(self: Pin<&mut EasimeSettings>);

        /// Look for the Japanese dictionary, once (not at start: it opens the
        /// files, tens of megabytes). "Reload" looks again.
        #[qinvokable]
        #[cxx_name = "refreshDictionary"]
        fn refresh_dictionary(self: Pin<&mut EasimeSettings>);

        #[qinvokable]
        #[cxx_name = "resetToDefaults"]
        fn reset_to_defaults(self: Pin<&mut EasimeSettings>);
    }
}

pub struct EasimeSettingsRust {
    activation_state: QString,
    activation_current: QString,
    activation_can_undo: bool,
    activation_undo_name: QString,
    activation_file: QString,
    languages: QString,
    languages_follow_system: bool,
    toggle_key: QString,
    backspace: QString,
    indicator_box: bool,
    indicator_ms: i32,
    problem_kind: QString,
    problem_detail: QString,
    result_kind: QString,
    result_detail: QString,
    dictionary_state: QString,
    dictionary_detail: QString,
    config_path: QString,
    log_path: QString,
    model: Model,
    activation: Activation,
}

impl Default for EasimeSettingsRust {
    fn default() -> Self {
        let model = Model::open(easime_config::config_path());
        let view = View::of(&model);
        let activation = Activation::from_env(&|name| std::env::var_os(name));
        let shown = ActivationView::of(&activation);
        let path_text = |path: Option<&std::path::Path>| {
            path.map(|p| QString::from(p.to_string_lossy().as_ref()))
                .unwrap_or_default()
        };
        Self {
            activation_state: QString::from(shown.state.as_str()),
            activation_current: QString::from(shown.current.as_str()),
            activation_can_undo: shown.can_undo,
            activation_undo_name: QString::from(shown.undo_name.as_str()),
            activation_file: QString::from(shown.file.as_str()),
            languages: QString::from(view.languages.as_str()),
            languages_follow_system: view.languages_follow_system,
            toggle_key: QString::from(view.toggle_key.as_str()),
            backspace: QString::from(view.backspace.as_str()),
            indicator_box: view.indicator_box,
            indicator_ms: view.indicator_ms,
            problem_kind: QString::from(view.problem_kind.as_str()),
            problem_detail: QString::from(view.problem_detail.as_str()),
            result_kind: QString::default(),
            result_detail: QString::default(),
            dictionary_state: QString::from("unknown"),
            dictionary_detail: QString::default(),
            config_path: path_text(model.path()),
            log_path: path_text(easime_config::log_path().as_deref()),
            model,
            activation,
        }
    }
}

/// What an action reports back to the pages.
enum Outcome {
    /// Saved (a change) or done (reload, reset, with an optional detail).
    Done(&'static str, String),
    Failed(Failure),
}

impl qobject::EasimeSettings {
    /// Run `action` on the model, show what it did, and refresh the pages.
    fn run(mut self: Pin<&mut Self>, action: impl FnOnce(&mut Model) -> Result<Outcome, Failure>) {
        // Start from a blank result: two "saved" in a row are still two changes
        // for the pages that react to them.
        self.as_mut().set_result_kind(QString::default());
        let outcome = {
            let rust = self.as_mut().rust_mut();
            let rust = rust.get_mut();
            action(&mut rust.model).unwrap_or_else(Outcome::Failed)
        };
        let (kind, detail) = match outcome {
            Outcome::Done(kind, detail) => (kind.to_owned(), detail),
            Outcome::Failed(failure) => (failure_code(failure.kind).to_owned(), failure.detail),
        };
        // The detail first: a page reads it when the kind changes.
        self.as_mut()
            .set_result_detail(QString::from(detail.as_str()));
        self.as_mut().set_result_kind(QString::from(kind.as_str()));
        self.refresh();
    }

    /// Put the model's current state into the properties.
    fn refresh(mut self: Pin<&mut Self>) {
        let view = View::of(&self.as_ref().rust().model);
        self.as_mut()
            .set_languages(QString::from(view.languages.as_str()));
        self.as_mut()
            .set_languages_follow_system(view.languages_follow_system);
        self.as_mut()
            .set_toggle_key(QString::from(view.toggle_key.as_str()));
        self.as_mut()
            .set_backspace(QString::from(view.backspace.as_str()));
        self.as_mut().set_indicator_box(view.indicator_box);
        self.as_mut().set_indicator_ms(view.indicator_ms);
        self.as_mut()
            .set_problem_kind(QString::from(view.problem_kind.as_str()));
        self.as_mut()
            .set_problem_detail(QString::from(view.problem_detail.as_str()));
    }

    fn set_language_enabled(self: Pin<&mut Self>, code: &QString, enabled: bool) {
        let language = language_from_code(&code.to_string());
        self.run(|model| {
            let language = language.ok_or_else(invalid)?;
            model.set_language_enabled(language, enabled)?;
            Ok(Outcome::Done("saved", String::new()))
        });
    }

    fn move_language(self: Pin<&mut Self>, code: &QString, up: bool) {
        let language = language_from_code(&code.to_string());
        self.run(|model| {
            let language = language.ok_or_else(invalid)?;
            model.move_language(language, up)?;
            Ok(Outcome::Done("saved", String::new()))
        });
    }

    fn set_toggle_key_code(self: Pin<&mut Self>, code: &QString) {
        let key = toggle_key_from_code(&code.to_string());
        self.run(|model| {
            model.set_toggle_key(key.ok_or_else(invalid)?)?;
            Ok(Outcome::Done("saved", String::new()))
        });
    }

    fn set_backspace_code(self: Pin<&mut Self>, code: &QString) {
        let unit = backspace_from_code(&code.to_string());
        self.run(|model| {
            model.set_backspace(unit.ok_or_else(invalid)?)?;
            Ok(Outcome::Done("saved", String::new()))
        });
    }

    fn set_indicator_box_shown(self: Pin<&mut Self>, shown: bool) {
        self.run(|model| {
            model.set_indicator_box(shown)?;
            Ok(Outcome::Done("saved", String::new()))
        });
    }

    fn set_indicator_milliseconds(self: Pin<&mut Self>, milliseconds: i32) {
        self.run(|model| {
            let milliseconds = u32::try_from(milliseconds).map_err(|_| invalid())?;
            model.set_indicator_ms(milliseconds)?;
            Ok(Outcome::Done("saved", String::new()))
        });
    }

    /// Show the state of the input method selection again.
    fn refresh_activation(mut self: Pin<&mut Self>) {
        let view = ActivationView::of(&self.as_ref().rust().activation);
        self.as_mut()
            .set_activation_state(QString::from(view.state.as_str()));
        self.as_mut()
            .set_activation_current(QString::from(view.current.as_str()));
        self.as_mut().set_activation_can_undo(view.can_undo);
        self.as_mut()
            .set_activation_undo_name(QString::from(view.undo_name.as_str()));
        self.as_mut()
            .set_activation_file(QString::from(view.file.as_str()));
    }

    /// Run an input method change, report it like any other action, and show
    /// the new state.
    fn run_activation(
        mut self: Pin<&mut Self>,
        done: &'static str,
        action: impl FnOnce(&Activation) -> Result<(), ActivationFailure>,
    ) {
        self.as_mut().set_result_kind(QString::default());
        let outcome = action(&self.as_ref().rust().activation);
        let (kind, detail) = match outcome {
            Ok(()) => (done.to_owned(), String::new()),
            Err(ActivationFailure::Changed) => (
                failure_code(FailureKind::ActivationChanged).to_owned(),
                String::new(),
            ),
            Err(ActivationFailure::NotPossible) => {
                (failure_code(FailureKind::Invalid).to_owned(), String::new())
            }
            Err(ActivationFailure::Io(detail)) => {
                (failure_code(FailureKind::WriteFailed).to_owned(), detail)
            }
        };
        self.as_mut()
            .set_result_detail(QString::from(detail.as_str()));
        self.as_mut().set_result_kind(QString::from(kind.as_str()));
        self.refresh_activation();
    }

    fn apply_activation(self: Pin<&mut Self>) {
        self.run_activation("activated", Activation::activate);
    }

    fn undo_activation(self: Pin<&mut Self>) {
        self.run_activation("deactivated", Activation::undo);
    }

    /// Forget what the last action reported, for example when the user moves
    /// to another page: "saved" belongs to the page where it was saved.
    fn clear_result(mut self: Pin<&mut Self>) {
        self.as_mut().set_result_kind(QString::default());
        self.as_mut().set_result_detail(QString::default());
    }

    fn refresh_dictionary(mut self: Pin<&mut Self>) {
        // The page asks each time it opens; reading the files again each time
        // would make switching pages stall.
        if self.dictionary_state().to_string() != "unknown" {
            return;
        }
        let state = crate::dictionary::inspect_installed();
        self.as_mut()
            .set_dictionary_detail(QString::from(state.detail().as_str()));
        self.as_mut()
            .set_dictionary_state(QString::from(state.code()));
    }

    fn reload(mut self: Pin<&mut Self>) {
        self.as_mut().run(|model| {
            model.reload();
            Ok(Outcome::Done("reloaded", String::new()))
        });
        // "Reload" also looks for the dictionary again (it may have been
        // installed while the window was open).
        self.as_mut().set_dictionary_state(QString::from("unknown"));
        self.refresh_dictionary();
    }

    fn reset_to_defaults(self: Pin<&mut Self>) {
        self.run(|model| {
            let backup = model.reset_to_defaults()?;
            let detail = backup
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(Outcome::Done("reset", detail))
        });
    }
}

fn invalid() -> Failure {
    Failure {
        kind: crate::model::FailureKind::Invalid,
        detail: String::new(),
    }
}
