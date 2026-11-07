//! Minimal file logger.
//!
//! KWin starts the input method itself, so stderr is easy to lose. Messages go
//! to the log file (`easime_config::log_path`: on Linux
//! `$XDG_STATE_HOME/easime/easime.log`, default `~/.local/state/easime/`),
//! which is truncated on every start.
//!
//! The path is shared with the settings window and the other OSes' frontends;
//! only the writing here is Linux/Wayland specific.

use std::fs::{self, File};
use std::io::Write;
use std::sync::{Mutex, OnceLock};

static LOG: OnceLock<Mutex<File>> = OnceLock::new();

/// Open the log file. Failure is ignored: logging is best effort.
pub fn init() {
    let Some(path) = easime_config::log_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(file) = File::create(&path) {
        let _ = LOG.set(Mutex::new(file));
    }
}

#[doc(hidden)]
pub fn write(args: std::fmt::Arguments<'_>) {
    if let Some(file) = LOG.get()
        && let Ok(mut file) = file.lock()
    {
        let _ = writeln!(file, "{args}");
        let _ = file.flush();
    }
}

/// `log!("fmt", args...)` appends one line to the log file.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => { $crate::log::write(format_args!($($arg)*)) };
}
