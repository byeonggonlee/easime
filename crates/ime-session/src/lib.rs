//! The part of an input method that does not depend on the operating system.
//!
//! A frontend (Wayland today; IBus, Windows TSF and macOS InputMethodKit
//! later) translates what its OS sends into [`ime_core::KeyEvent`]s, hands them
//! to a [`Session`], and puts what comes back (text to commit, the composition,
//! a candidate list) on screen the way its OS wants. Everything that decides
//! *what* happens lives here, so the same typing gives the same result on every
//! OS:
//!
//! * [`Session`]: the engines for the enabled languages, the cycle between
//!   them, and the rules for switching language, changing settings and losing
//!   focus without losing text.
//! * [`toggle`]: which presses of the switch key (Hangul key, tapped right Alt)
//!   switch language.
//! * [`dictionary`]: the Japanese dictionary, opened once and shared.
//! * [`indicator`]: the letter that stands for a language.
//! * [`reload`]: noticing a changed settings file without an OS watcher API (for
//!   frontends that cannot or should not watch files).
//!
//! What stays in the frontend, and which parts differ between operating
//! systems, is listed in `docs/platforms.md`. This crate must stay free of OS
//! and UI dependencies (the CI builds it on Windows and macOS too).

pub mod dictionary;
pub mod indicator;
pub mod reload;
pub mod session;
pub mod toggle;

pub use dictionary::JapaneseDictionary;
pub use reload::ConfigReloader;
pub use session::{LogFn, Session};
pub use toggle::{Decision, Held, KeyRole, ToggleTracker};
