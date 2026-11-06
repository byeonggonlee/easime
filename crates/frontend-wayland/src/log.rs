//! Minimal file logger.
//!
//! KWin starts the input method itself, so stderr is easy to lose. Messages go
//! to `$XDG_STATE_HOME/easime/easime.log` (default `~/.local/state/easime/`),
//! which is truncated on every start.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static LOG: OnceLock<Mutex<File>> = OnceLock::new();

/// Pick the state directory per the XDG spec: `$XDG_STATE_HOME` if it is an
/// absolute path, else `$HOME/.local/state`. Empty or relative values are ignored.
fn state_dir(xdg_state_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    xdg_state_home
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| PathBuf::from(h).join(".local/state")))
}

fn log_path() -> Option<PathBuf> {
    let base = state_dir(std::env::var_os("XDG_STATE_HOME"), std::env::var_os("HOME"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn os(s: &str) -> Option<OsString> {
        Some(OsString::from(s))
    }

    #[test]
    fn uses_absolute_xdg_state_home() {
        assert_eq!(
            state_dir(os("/data/state"), os("/home/u")),
            Some(PathBuf::from("/data/state"))
        );
    }

    #[test]
    fn ignores_empty_or_relative_xdg_state_home() {
        let expected = Some(PathBuf::from("/home/u/.local/state"));
        assert_eq!(state_dir(os(""), os("/home/u")), expected);
        assert_eq!(state_dir(os("relative/dir"), os("/home/u")), expected);
        assert_eq!(state_dir(None, os("/home/u")), expected);
    }

    #[test]
    fn none_without_any_base() {
        assert_eq!(state_dir(None, None), None);
    }
}
