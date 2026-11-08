//! The part of the easime settings apps that has no UI toolkit in it, shared
//! by every OS's app (KDE: `settings/kde`, Qt and Kirigami; Windows:
//! `settings/windows`, WinUI 3), so a change is saved the same way everywhere
//! and is tested on every OS.
//!
//! - [`model`]: the settings as the pages show them, and what a change does to
//!   the settings file (only the keys that change are rewritten; a file that
//!   cannot be read is never overwritten).
//! - [`dictionary`]: whether the Japanese dictionary is there.
//!
//! Words the user reads are not here: each app translates them itself.

pub mod dictionary;
pub mod model;
