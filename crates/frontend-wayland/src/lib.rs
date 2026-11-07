//! Wayland frontend: connects the easime engines to KWin.
//!
//! Only the Linux/Wayland specifics are here: the protocol, key translation, our
//! own key repeat, the windows that show the indicator and the candidates. The
//! typing logic is shared with every other frontend in `ime-session`. Which
//! module is OS-specific and what another OS provides instead is in
//! `docs/platforms.md`.

pub mod app;
mod candidate;
pub mod font;
pub mod indicator;
pub mod log;
mod panel;
pub mod protocol;
pub mod repeat;
pub mod timer;
pub mod translate;
pub mod watch;

pub use app::{RunError, run, run_on, run_watching, run_with, run_with_dictionary};
