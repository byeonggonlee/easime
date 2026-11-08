//! What the text service says about itself, for the person who has to find out
//! why a key did not do what they expected: lines in `easime.log`
//! ([`easime_config::log_path`]).
//!
//! The service runs inside other programs, so it writes little: what it was
//! started with, errors, and every language switch. Set the environment
//! variable `EASIME_LOG_KEYS=1` (and restart the program) to also log every key
//! and what was done with it.

use std::io::Write;
use std::sync::OnceLock;

/// Append a line to the log (the temporary folder if the process has no home,
/// as an installer's helper process may not). A log that cannot be written is
/// not a reason to fail anything.
pub fn log(line: &str) {
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
        let _ = writeln!(file, "[{}] {line}", std::process::id());
    }
}

/// Whether every key is logged (`EASIME_LOG_KEYS` is set to something but 0).
pub fn logging_keys() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("EASIME_LOG_KEYS").is_ok_and(|v| !v.is_empty() && v != "0"))
}

/// Log a line about a key, if keys are being logged.
pub fn log_key(line: impl FnOnce() -> String) {
    if logging_keys() {
        log(&line());
    }
}
