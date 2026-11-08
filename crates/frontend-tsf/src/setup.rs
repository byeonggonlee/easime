//! The steps the installer runs after the machine-wide registration: adding
//! easime's profile of a language to the input methods of the user who installs
//! it, and taking it away on uninstall.
//!
//! The installer calls them through `rundll32 easime_tsf.dll,EnableForCurrentUser
//! korean` (the exports are in `lib.rs`). `rundll32` always exits with 0, so what
//! happened is written to the log file (`easime.log`, see
//! [`easime_config::log_path`]), and the installer's own checks read the user's
//! language list afterwards. Anyone can run the same commands by hand to add or
//! remove one language.

use crate::{profiles, registration};
use std::io::Write;

/// What to do with the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Enable,
    Disable,
}

impl Step {
    fn verb(self) -> &'static str {
        match self {
            Step::Enable => "enable",
            Step::Disable => "disable",
        }
    }
}

/// Run `step` for the language named on the command line (`korean`,
/// `japanese`, ...; see [`profiles::language_named`]). Returns whether it
/// worked, and logs what happened either way.
pub fn run(step: Step, command_line: &str) -> bool {
    let name = command_line.trim().trim_matches('"').trim();
    let outcome = match profiles::language_named(name) {
        None => Err(format!("'{name}' is not a language easime knows")),
        Some(language) => match step {
            Step::Enable => registration::enable_for_current_user(language),
            Step::Disable => registration::disable_for_current_user(language),
        }
        .map_err(|e| e.to_string()),
    };
    match &outcome {
        Ok(()) => log(&format!("setup: {} {name}: done", step.verb())),
        Err(why) => log(&format!("setup: {} {name}: FAILED: {why}", step.verb())),
    }
    outcome.is_ok()
}

/// Append a line to the log (the temporary folder if the process has no home,
/// as an installer's helper process may not). A log that cannot be written is
/// not a reason to fail the installer.
fn log(line: &str) {
    let path =
        easime_config::log_path().unwrap_or_else(|| std::env::temp_dir().join("easime-setup.log"));
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}
