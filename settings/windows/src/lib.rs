//! The easime settings app for Windows: WinUI 3 through `windows-reactor`,
//! with the same pages, saving rules and words as the KDE window
//! (`settings/kde`). Only the window is Windows code ([`app`], Windows only);
//! the words ([`tr`]) and where the installed files are ([`install`]) are
//! checked on every OS.

#[cfg(windows)]
pub mod app;
pub mod install;
pub mod tr;
