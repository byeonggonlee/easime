//! Watch the settings file so changes apply without restarting.
//!
//! **OS-specific: Linux** (`inotify`). Windows would use `ReadDirectoryChangesW`, macOS
//! FSEvents or kqueue (`docs/platforms.md`).

use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};
use rustix::io::Errno;
use std::ffi::OsString;
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// Watches one file by watching its directory, which also catches the atomic
/// "write a temp file, rename over" saves that easime-config uses.
pub struct ConfigWatcher {
    fd: OwnedFd,
    file_name: OsString,
}

impl ConfigWatcher {
    /// Start watching `path`. Its directory is created if it does not exist yet.
    pub fn new(path: &Path) -> io::Result<Self> {
        let dir = path
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no parent directory"))?;
        let file_name = path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?
            .to_os_string();
        std::fs::create_dir_all(dir)?;
        let fd = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK)?;
        inotify::add_watch(
            &fd,
            dir,
            WatchFlags::CLOSE_WRITE
                | WatchFlags::MOVED_TO
                | WatchFlags::MOVED_FROM
                | WatchFlags::CREATE
                | WatchFlags::DELETE,
        )?;
        Ok(Self { fd, file_name })
    }

    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    /// Drain pending events. True if the watched file was touched.
    pub fn take_changed(&self) -> io::Result<bool> {
        let mut buf = [MaybeUninit::<u8>::uninit(); 4096];
        let mut reader = inotify::Reader::new(&self.fd, &mut buf);
        let mut changed = false;
        loop {
            match reader.next() {
                Ok(event) => {
                    let ours = event
                        .file_name()
                        .is_some_and(|n| n.to_bytes() == self.file_name.as_bytes());
                    if ours && !event.events().contains(ReadFlags::ISDIR) {
                        changed = true;
                    }
                }
                Err(Errno::AGAIN) => return Ok(changed),
                Err(e) => return Err(e.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::event::{PollFd, PollFlags, poll};
    use rustix::time::Timespec;
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("easime-watch-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn readable(w: &ConfigWatcher, timeout: Duration) -> bool {
        let mut fds = [PollFd::new(&w.fd, PollFlags::IN)];
        let ts = Timespec {
            tv_sec: timeout.as_secs() as _,
            tv_nsec: timeout.subsec_nanos() as _,
        };
        matches!(poll(&mut fds, Some(&ts)), Ok(n) if n > 0)
    }

    #[test]
    fn creates_the_directory_and_sees_a_new_file() {
        let dir = scratch("create").join("nested");
        let path = dir.join("config.toml");
        let w = ConfigWatcher::new(&path).unwrap();
        assert!(dir.is_dir());
        assert!(!w.take_changed().unwrap());

        fs::write(&path, "x = 1").unwrap();
        assert!(readable(&w, Duration::from_secs(2)));
        assert!(w.take_changed().unwrap());
        assert!(!w.take_changed().unwrap(), "events are drained");
        fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn sees_an_atomic_rename_over_the_file() {
        let dir = scratch("rename");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(&path, "a").unwrap();
        let w = ConfigWatcher::new(&path).unwrap();

        let tmp = dir.join("config.toml.tmp");
        fs::write(&tmp, "b").unwrap();
        fs::rename(&tmp, &path).unwrap();
        assert!(readable(&w, Duration::from_secs(2)));
        assert!(w.take_changed().unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ignores_other_files_in_the_directory() {
        let dir = scratch("other");
        fs::create_dir_all(&dir).unwrap();
        let w = ConfigWatcher::new(&dir.join("config.toml")).unwrap();

        fs::write(dir.join("config.toml.tmp"), "b").unwrap();
        fs::write(dir.join("unrelated.txt"), "c").unwrap();
        // Events arrive, but none is for our file.
        assert!(readable(&w, Duration::from_secs(2)));
        assert!(!w.take_changed().unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sees_deletion() {
        let dir = scratch("delete");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(&path, "a").unwrap();
        let w = ConfigWatcher::new(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(readable(&w, Duration::from_secs(2)));
        assert!(w.take_changed().unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }
}
