//! Wayland frontend: connects the easime engines to KWin.
//!
//! Only the Linux/Wayland specifics are here: the protocol, key translation, our
//! own key repeat, the windows that show the indicator and the candidates. The
//! typing logic is shared with every other frontend in `ime-session`. Which
//! module is OS-specific and what another OS provides instead is in
//! `docs/platforms.md`.

pub mod app;
mod candidate;
mod control;
pub mod font;
pub mod indicator;
pub mod log;
mod panel;
pub mod protocol;
pub mod repeat;
pub mod timer;
pub mod translate;
pub mod watch;

pub use app::{
    RunError, observe_language, run, run_on, run_watching, run_with, run_with_dictionary,
};

/// Ask the running input method to switch to `language` (ignored if it is not
/// one of the cycle). Callable from any thread; the input method does it on its
/// own thread, finishing the text in progress first.
pub fn select_language(language: ime_core::Language) {
    control::send(control::Command::Select(language));
}

/// Ask the running input method to switch to the next language, like the
/// switch key.
pub fn next_language() {
    control::send(control::Command::Next);
}
