//! Noticing that the settings file changed, without any OS file-watching API.
//!
//! Linux watches the file with inotify and reacts at once. Other frontends can
//! use this instead (a Windows text service lives inside every application, so
//! a watcher per process would be wasteful): they call [`ConfigReloader::poll`]
//! when something happens anyway (a key press, a focus change) and it looks at
//! the file at most once per interval. A change is therefore noticed on the
//! next key after the interval, which is as soon as it can matter.

use easime_config::{Config, StoreError};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

/// What identifies one version of the file. The settings app replaces the file
/// as a whole (write a temporary file, rename over), which changes the time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
}

pub struct ConfigReloader {
    path: PathBuf,
    interval: Duration,
    /// The file as it was when last looked at; `None` if it did not exist.
    seen: Option<Stamp>,
    checked: Option<Instant>,
}

impl ConfigReloader {
    /// Watch `path`, looking at it at most once per `interval`. The file as it
    /// is now counts as already loaded: only later changes are reported.
    pub fn new(path: PathBuf, interval: Duration) -> Self {
        let seen = stamp(&path);
        Self {
            path,
            interval,
            seen,
            checked: None,
        }
    }

    /// If the file changed since it was last looked at, load it. `now` is the
    /// current time (passed in so tests need no clock). A file that cannot be
    /// read or understood is reported as an error and **not** retried until it
    /// changes again: the caller keeps its current settings, like a half-edited
    /// file must not break typing.
    pub fn poll(&mut self, now: Instant) -> Option<Result<Config, StoreError>> {
        if let Some(last) = self.checked
            && now.saturating_duration_since(last) < self.interval
        {
            return None;
        }
        self.checked = Some(now);
        let current = stamp(&self.path);
        if current == self.seen {
            return None;
        }
        self.seen = current;
        Some(Config::load_from(&self.path))
    }
}

fn stamp(path: &PathBuf) -> Option<Stamp> {
    let meta = fs::metadata(path).ok()?;
    Some(Stamp {
        modified: meta.modified().ok(),
        len: meta.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use easime_config::ToggleKey;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Dir(PathBuf);

    impl Dir {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "easime-reload-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join("config.toml")
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    const EVERY_TIME: Duration = Duration::ZERO;

    #[test]
    fn the_file_as_it_is_now_is_not_a_change() {
        let dir = Dir::new();
        fs::write(dir.file(), "[shortcuts]\ntoggle_key = \"hangul\"\n").unwrap();
        let mut reloader = ConfigReloader::new(dir.file(), EVERY_TIME);
        assert!(reloader.poll(Instant::now()).is_none());
    }

    #[test]
    fn a_changed_file_is_loaded_once() {
        let dir = Dir::new();
        fs::write(dir.file(), "").unwrap();
        let mut reloader = ConfigReloader::new(dir.file(), EVERY_TIME);
        fs::write(dir.file(), "[shortcuts]\ntoggle_key = \"right_alt\"\n").unwrap();
        let config = reloader.poll(Instant::now()).unwrap().unwrap();
        assert_eq!(config.shortcuts.toggle_key, ToggleKey::RightAlt);
        assert!(reloader.poll(Instant::now()).is_none(), "reported once");
    }

    #[test]
    fn a_file_that_appears_or_goes_away_is_a_change() {
        let dir = Dir::new();
        let mut reloader = ConfigReloader::new(dir.file(), EVERY_TIME);
        assert!(reloader.poll(Instant::now()).is_none(), "still no file");
        fs::write(dir.file(), "[shortcuts]\ntoggle_key = \"hangul\"\n").unwrap();
        let config = reloader.poll(Instant::now()).unwrap().unwrap();
        assert_eq!(config.shortcuts.toggle_key, ToggleKey::Hangul);
        fs::remove_file(dir.file()).unwrap();
        let config = reloader.poll(Instant::now()).unwrap().unwrap();
        assert_eq!(config, Config::default(), "no file means the defaults");
    }

    #[test]
    fn a_broken_file_is_reported_once_and_not_retried() {
        let dir = Dir::new();
        fs::write(dir.file(), "").unwrap();
        let mut reloader = ConfigReloader::new(dir.file(), EVERY_TIME);
        fs::write(dir.file(), "this is = = not toml").unwrap();
        assert!(reloader.poll(Instant::now()).unwrap().is_err());
        assert!(reloader.poll(Instant::now()).is_none());
        // Fixing it is a new change.
        fs::write(dir.file(), "[shortcuts]\ntoggle_key = \"hangul\"\n").unwrap();
        assert!(reloader.poll(Instant::now()).unwrap().is_ok());
    }

    #[test]
    fn the_file_is_only_looked_at_once_per_interval() {
        let dir = Dir::new();
        fs::write(dir.file(), "").unwrap();
        let interval = Duration::from_secs(1);
        let mut reloader = ConfigReloader::new(dir.file(), interval);
        let start = Instant::now();
        assert!(reloader.poll(start).is_none());
        fs::write(dir.file(), "[shortcuts]\ntoggle_key = \"hangul\"\n").unwrap();
        assert!(
            reloader.poll(start + interval / 2).is_none(),
            "too soon to look again"
        );
        assert!(
            reloader.poll(start + interval).unwrap().is_ok(),
            "the change is found as soon as the interval is over"
        );
    }
}
