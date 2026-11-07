//! Wayland frontend: connects the easime engines to KWin.

pub mod app;
mod candidate;
pub mod font;
pub mod indicator;
pub mod log;
mod panel;
pub mod protocol;
pub mod repeat;
pub mod timer;
pub mod toggle;
pub mod translate;
pub mod watch;

pub use app::{RunError, run, run_on, run_watching, run_with};
