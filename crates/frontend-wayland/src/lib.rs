//! Wayland frontend: connects the easime engines to KWin.

pub mod app;
pub mod log;
pub mod protocol;
pub mod repeat;
pub mod timer;
pub mod toggle;
pub mod translate;

pub use app::{RunError, run, run_on, run_with};
