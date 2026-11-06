//! Minimal file logger.
//!
//! KWin starts the input method itself, so stderr is easy to lose. Messages go
//! to `$XDG_STATE_HOME/easime/easime.log` (default `~/.local/state/easime/`),
//! which is truncated on every start.

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static LOG: OnceLock<Mutex<File>> = OnceLock::new();

fn log_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("easime").join("easime.log"))
}

/// Open the log file. Failure is ignored: logging is best effort.
pub fn init() {
    let Some(path) = log_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(file) = File::create(&path) {
        let _ = LOG.set(Mutex::new(file));
    }
}

#[doc(hidden)]
pub fn write(args: std::fmt::Arguments<'_>) {
    if let Some(file) = LOG.get() {
        if let Ok(mut file) = file.lock() {
            let _ = writeln!(file, "{args}");
            let _ = file.flush();
        }
    }
}

/// `log!("fmt", args...)` appends one line to the log file.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => { $crate::log::write(format_args!($($arg)*)) };
}
