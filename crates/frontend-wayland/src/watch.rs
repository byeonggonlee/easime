//! Watch the settings file so changes apply without restarting.
//!
//! **OS-specific: Linux** (`inotify`). Windows would use `ReadDirectoryChangesW`, macOS
//! FSEvents or kqueue (`docs/platforms.md`).

use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};
use rustix::io::Errno;
use std::ffi::{OsStr, OsString};
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Watches one file by watching its directory, which also catches the atomic
/// "write a temp file, rename over" saves that easime-config uses.
///
/// When the file is a symbolic link (dotfile managers do that), the settings
/// window saves into the file the link points to, not into the link's own
/// directory. That directory is watched too, for that one file name, and the
/// link is looked at again whenever it changes itself.
pub struct ConfigWatcher {
    fd: OwnedFd,
    /// The file as it was named.
    path: PathBuf,
    /// What counts as the file: a watched directory (its watch descriptor) and
    /// the name inside it. The first entry is `path` itself.
    files: Vec<(i32, OsString)>,
}

impl ConfigWatcher {
    /// Start watching `path`. Its directory is created if it does not exist yet.
    pub fn new(path: &Path) -> io::Result<Self> {
        let dir = path
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no parent directory"))?;
        let file_name = path
            .file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
        std::fs::create_dir_all(dir)?;
        let fd = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK)?;
        let mut watcher = Self {
            fd,
            path: path.to_path_buf(),
            files: Vec::new(),
        };
        watcher.watch(dir, file_name)?;
        watcher.follow_link();
        Ok(watcher)
    }

    pub fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    /// Count changes to `name` in `dir`.
    fn watch(&mut self, dir: &Path, name: &OsStr) -> io::Result<()> {
        let descriptor = inotify::add_watch(
            &self.fd,
            dir,
            WatchFlags::CLOSE_WRITE
                | WatchFlags::MOVED_TO
                | WatchFlags::MOVED_FROM
                | WatchFlags::CREATE
                | WatchFlags::DELETE,
        )?;
        if !self
            .files
            .iter()
            .any(|(d, n)| *d == descriptor && n == name)
        {
            self.files.push((descriptor, name.to_os_string()));
        }
        Ok(())
    }

    /// If the file is a link, also watch the file it points to. Not being able
    /// to is not an error: the link itself is still watched.
    fn follow_link(&mut self) {
        let is_link =
            std::fs::symlink_metadata(&self.path).is_ok_and(|m| m.file_type().is_symlink());
        if !is_link {
            return;
        }
        let Ok(real) = std::fs::canonicalize(&self.path) else {
            return;
        };
        if let (Some(dir), Some(name)) = (real.parent(), real.file_name()) {
            let _ = self.watch(dir, name);
        }
    }

    /// Drain pending events. True if the watched file was touched.
    pub fn take_changed(&mut self) -> io::Result<bool> {
        let mut buf = [MaybeUninit::<u8>::uninit(); 4096];
        let mut changed = false;
        let mut link_touched = false;
        let result = {
            let mut reader = inotify::Reader::new(&self.fd, &mut buf);
            loop {
                match reader.next() {
                    Ok(event) => {
                        let Some(name) = event.file_name() else {
                            continue;
                        };
                        if event.events().contains(ReadFlags::ISDIR) {
                            continue;
                        }
                        let ours = |entry: &(i32, OsString)| {
                            entry.0 == event.wd() && entry.1.as_bytes() == name.to_bytes()
                        };
                        if self.files.iter().any(ours) {
                            changed = true;
                            link_touched |= self.files.first().is_some_and(ours);
                        }
                    }
                    Err(Errno::AGAIN) => break Ok(changed),
                    Err(e) => break Err(e.into()),
                }
            }
        };
        if link_touched {
            // Replaced by a link, or the link now points elsewhere.
            self.follow_link();
        }
        result
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
        let mut w = ConfigWatcher::new(&path).unwrap();
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
        let mut w = ConfigWatcher::new(&path).unwrap();

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
        let mut w = ConfigWatcher::new(&dir.join("config.toml")).unwrap();

        fs::write(dir.join("config.toml.tmp"), "b").unwrap();
        fs::write(dir.join("unrelated.txt"), "c").unwrap();
        // Events arrive, but none is for our file.
        assert!(readable(&w, Duration::from_secs(2)));
        assert!(!w.take_changed().unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// How the settings window saves: a temporary file beside the real file,
    /// renamed over it.
    fn save_over(target: &Path, text: &str) {
        let mut tmp = target.as_os_str().to_owned();
        tmp.push(".tmp");
        fs::write(&tmp, text).unwrap();
        fs::rename(&tmp, target).unwrap();
    }

    #[test]
    fn sees_a_save_in_the_folder_a_link_points_to() {
        // Dotfile managers link the settings file to a file in their repository.
        let root = scratch("link-elsewhere");
        let (config, repository) = (root.join("config"), root.join("dotfiles"));
        fs::create_dir_all(&config).unwrap();
        fs::create_dir_all(&repository).unwrap();
        let target = repository.join("easime.toml");
        fs::write(&target, "a").unwrap();
        let link = config.join("config.toml");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let mut w = ConfigWatcher::new(&link).unwrap();

        save_over(&target, "b");
        assert!(readable(&w, Duration::from_secs(2)));
        assert!(w.take_changed().unwrap());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn sees_a_save_under_the_other_name_of_a_link_in_the_same_folder() {
        let dir = scratch("link-beside");
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("real.toml");
        fs::write(&target, "a").unwrap();
        let link = dir.join("config.toml");
        std::os::unix::fs::symlink("real.toml", &link).unwrap();
        let mut w = ConfigWatcher::new(&link).unwrap();

        save_over(&target, "b");
        assert!(readable(&w, Duration::from_secs(2)));
        assert!(w.take_changed().unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn follows_a_link_made_or_moved_while_running() {
        let root = scratch("link-later");
        let (config, first, second) =
            (root.join("config"), root.join("first"), root.join("second"));
        for dir in [&config, &first, &second] {
            fs::create_dir_all(dir).unwrap();
        }
        let link = config.join("config.toml");
        fs::write(&link, "plain file").unwrap();
        let mut w = ConfigWatcher::new(&link).unwrap();

        // The plain file is replaced by a link: that is a change in itself...
        let (one, two) = (first.join("easime.toml"), second.join("easime.toml"));
        fs::write(&one, "a").unwrap();
        fs::write(&two, "b").unwrap();
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(&one, &link).unwrap();
        assert!(readable(&w, Duration::from_secs(2)));
        assert!(w.take_changed().unwrap());
        // ...and from then on a save to the file it points to is seen,
        save_over(&one, "c");
        assert!(readable(&w, Duration::from_secs(2)));
        assert!(w.take_changed().unwrap());
        // also after the link is moved to another file.
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(&two, &link).unwrap();
        assert!(readable(&w, Duration::from_secs(2)));
        assert!(w.take_changed().unwrap());
        save_over(&two, "d");
        assert!(readable(&w, Duration::from_secs(2)));
        assert!(w.take_changed().unwrap());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn ignores_other_files_in_the_folder_a_link_points_to() {
        let root = scratch("link-other");
        let (config, repository) = (root.join("config"), root.join("dotfiles"));
        fs::create_dir_all(&config).unwrap();
        fs::create_dir_all(&repository).unwrap();
        let target = repository.join("easime.toml");
        fs::write(&target, "a").unwrap();
        let link = config.join("config.toml");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let mut w = ConfigWatcher::new(&link).unwrap();

        fs::write(repository.join("README.md"), "c").unwrap();
        assert!(readable(&w, Duration::from_secs(2)));
        assert!(!w.take_changed().unwrap());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn sees_deletion() {
        let dir = scratch("delete");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(&path, "a").unwrap();
        let mut w = ConfigWatcher::new(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(readable(&w, Duration::from_secs(2)));
        assert!(w.take_changed().unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }
}
