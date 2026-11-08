//! The steps the installer runs after the machine-wide registration: switching
//! easime on in the input methods of the user who installs it, and taking it
//! away on uninstall.
//!
//! The installer calls them through `rundll32 easime_tsf.dll,EnableForCurrentUser`
//! and `...,DisableForCurrentUser` (the exports are in `lib.rs`). easime is
//! switched on under Korean, which Windows adds to the user's languages if they
//! do not have it ([`crate::profiles`]). `rundll32` always exits with 0, so what happened is written to the log file (`easime.log`, see
//! [`easime_config::log_path`]), and the installer's own checks read the user's
//! language list afterwards. Anyone can run the same commands by hand.

use crate::diag::log;
use crate::registration;

/// What to do for the current user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Enable,
    Disable,
}

/// Run `step` (a command line, if any, is ignored: earlier builds took a
/// language name there). Returns whether it worked, and logs what happened
/// either way.
pub fn run(step: Step, command_line: &str) -> bool {
    let ignored = command_line.trim().trim_matches('"').trim();
    if !ignored.is_empty() {
        log(&format!(
            "setup: ignoring '{ignored}': easime has one profile, under Korean"
        ));
    }
    let outcome = match step {
        Step::Enable => registration::enable_for_current_user()
            .map(|language| format!("enable under {language:04X}: done")),
        Step::Disable => registration::disable_for_current_user()
            .map(|count| format!("disable everywhere: done ({count} entries)")),
    };
    match &outcome {
        Ok(what) => log(&format!("setup: {what}")),
        Err(why) => {
            let verb = match step {
                Step::Enable => "enable",
                Step::Disable => "disable",
            };
            log(&format!("setup: {verb} FAILED: {why}"));
        }
    }
    outcome.is_ok()
}
