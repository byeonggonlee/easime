//! The Windows frontend: easime as a TSF (Text Services Framework) text
//! service.
//!
//! **What is here today** is the part that needs no Windows API and is checked
//! on every OS: turning what Windows reports about a key press into the
//! OS-independent [`ime_core::KeyEvent`] and the switch-key role the shared
//! `ime-session` wants ([`keys`]), and the list of what is registered with
//! Windows, one profile per language ([`profiles`]). **What is not here yet** is everything that
//! talks to TSF (the COM text service, composition, the candidate window,
//! registration); there is no Windows input method until that exists. See
//! `docs/platforms.md`, "Windows".

pub mod keys;
pub mod profiles;
