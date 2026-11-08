//! Making easime the input method KDE uses.
//!
//! Installing the package does not select easime: the choice is KDE's
//! "virtual keyboard" setting (`[Wayland] InputMethod` in `kwinrc`, the path of
//! the input method's `.desktop` file), and changing a user's desktop setting
//! needs their confirmation and a way back. So this module only *describes*
//! the change ([`Activation::plan`]), and applies it when the user has said
//! yes ([`Activation::activate`]), after saving the value it replaces so that
//! [`Activation::undo`] can put it back. An undo never overwrites a choice the
//! user made since: it only restores what it replaced, and only while easime is
//! still the selected input method.
//!
//! No Qt in here, so all of it is tested without a display.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The group and key of KWin's input method setting in `kwinrc`.
const GROUP: &str = "Wayland";
const KEY: &str = "InputMethod";
/// easime's own input method file, as the packages install it.
const DESKTOP_FILE: &str = "easime.desktop";

pub type Env<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// Where things stand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Not KDE Plasma: there is nothing to select here.
    Unsupported,
    /// easime's `.desktop` file is nowhere to be found: not installed properly.
    NotInstalled,
    /// Another input method, or none, is selected.
    Other {
        /// What is selected (the file's name without `.desktop`), if anything.
        current: Option<String>,
    },
    /// easime is selected. `running` says whether the daemon is running now
    /// (otherwise KWin starts it at the next login).
    Selected { running: bool, can_undo: bool },
}

/// What a change would do, to show before asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The file that is changed.
    pub file: PathBuf,
    /// The input method selected now, by name.
    pub from: Option<String>,
    /// What it becomes: easime's `.desktop` file.
    pub to: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The change is not possible in this state (already selected, not KDE,
    /// not installed, nothing to undo).
    NotPossible,
    /// Another input method was selected since: nothing was changed back.
    Changed,
    /// A file could not be read or written.
    Io(String),
}

pub struct Activation {
    kde: bool,
    kwinrc: Option<PathBuf>,
    desktop_file: Option<PathBuf>,
    /// Where the replaced value is kept for the undo.
    backup: Option<PathBuf>,
    running: Box<dyn Fn() -> bool>,
}

impl Activation {
    pub fn new(
        kde: bool,
        kwinrc: Option<PathBuf>,
        desktop_file: Option<PathBuf>,
        backup: Option<PathBuf>,
        running: Box<dyn Fn() -> bool>,
    ) -> Self {
        Self {
            kde,
            kwinrc,
            desktop_file,
            backup,
            running,
        }
    }

    /// For this machine: KDE or not, the files, whether the daemon runs.
    pub fn from_env(env: Env) -> Self {
        let backup = easime_config::config_path_for(ime_core::Os::current(), env)
            .and_then(|path| Some(path.parent()?.join("activation-backup.toml")));
        Self::new(
            is_kde(env),
            kwinrc_path(env),
            find_desktop_file(env),
            backup,
            Box::new(|| process_named(Path::new("/proc"), "easime")),
        )
    }

    #[cfg(test)]
    pub fn set_kde_for_test(&mut self, kde: bool) {
        self.kde = kde;
    }

    /// The `kwinrc` file the change goes to, if its place is known.
    pub fn kwinrc(&self) -> Option<&Path> {
        self.kwinrc.as_deref()
    }

    pub fn state(&self) -> State {
        if !self.kde {
            return State::Unsupported;
        }
        let Some(desktop_file) = &self.desktop_file else {
            return State::NotInstalled;
        };
        let current = self.current();
        match current {
            Some(value) if is_easime(&value, desktop_file) => State::Selected {
                running: (self.running)(),
                can_undo: self.read_backup().is_some(),
            },
            other => State::Other {
                current: other.as_deref().map(name_of),
            },
        }
    }

    /// What activating would change, or `None` if it cannot be done now.
    pub fn plan(&self) -> Option<Plan> {
        let State::Other { current } = self.state() else {
            return None;
        };
        Some(Plan {
            file: self.kwinrc.clone()?,
            from: current,
            to: self.desktop_file.clone()?,
        })
    }

    /// Select easime, keeping what it replaces. The user has confirmed.
    pub fn activate(&self) -> Result<(), Failure> {
        let (Some(plan), Some(kwinrc), Some(backup)) = (self.plan(), &self.kwinrc, &self.backup)
        else {
            return Err(Failure::NotPossible);
        };
        let text = read_or_empty(kwinrc)?;
        let previous = ini_get(&text, GROUP, KEY);
        write_backup(backup, previous.as_deref())?;
        let changed = ini_set(&text, GROUP, KEY, Some(&plan.to.to_string_lossy()));
        if let Err(e) = write_atomic(kwinrc, &changed) {
            let _ = fs::remove_file(backup);
            return Err(e);
        }
        Ok(())
    }

    /// What undoing would restore (the name of the input method that was
    /// selected before), if there is something to undo. `Some(None)` means
    /// "no input method was selected".
    pub fn undo_target(&self) -> Option<Option<String>> {
        match self.state() {
            State::Selected { can_undo: true, .. } => self
                .read_backup()
                .map(|value| value.as_deref().map(name_of)),
            _ => None,
        }
    }

    /// Put back what [`Activation::activate`] replaced, if easime is still the
    /// selected input method.
    pub fn undo(&self) -> Result<(), Failure> {
        let (Some(kwinrc), Some(backup), Some(desktop_file)) =
            (&self.kwinrc, &self.backup, &self.desktop_file)
        else {
            return Err(Failure::NotPossible);
        };
        let Some(previous) = self.read_backup() else {
            return Err(Failure::NotPossible);
        };
        let text = read_or_empty(kwinrc)?;
        match ini_get(&text, GROUP, KEY) {
            Some(value) if is_easime(&value, desktop_file) => {}
            _ => return Err(Failure::Changed),
        }
        let restored = ini_set(&text, GROUP, KEY, previous.as_deref());
        write_atomic(kwinrc, &restored)?;
        let _ = fs::remove_file(backup);
        Ok(())
    }

    fn current(&self) -> Option<String> {
        let text = fs::read_to_string(self.kwinrc.as_ref()?).ok()?;
        ini_get(&text, GROUP, KEY)
    }

    /// What was selected before easime: `Some(value)` or `Some(None)` for "no
    /// input method was set"; `None` if there is no backup.
    fn read_backup(&self) -> Option<Option<String>> {
        let text = fs::read_to_string(self.backup.as_ref()?).ok()?;
        let doc: toml_edit::DocumentMut = text.parse().ok()?;
        if doc.get("had_value")?.as_bool()? {
            Some(Some(doc.get("previous")?.as_str()?.to_owned()))
        } else {
            Some(None)
        }
    }
}

/// True if `value` (what `kwinrc` says) is easime's input method.
fn is_easime(value: &str, desktop_file: &Path) -> bool {
    Path::new(value) == desktop_file
        || Path::new(value)
            .file_name()
            .is_some_and(|name| name == DESKTOP_FILE)
}

/// `/usr/share/applications/org.kde.plasma.keyboard.desktop` →
/// `org.kde.plasma.keyboard`.
fn name_of(value: &str) -> String {
    let file = Path::new(value)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| value.to_owned());
    file.strip_suffix(".desktop").unwrap_or(&file).to_owned()
}

fn write_backup(path: &Path, previous: Option<&str>) -> Result<(), Failure> {
    let mut doc = toml_edit::DocumentMut::new();
    doc["had_value"] = toml_edit::value(previous.is_some());
    if let Some(previous) = previous {
        doc["previous"] = toml_edit::value(previous);
    }
    write_atomic(path, &doc.to_string())
}

fn read_or_empty(path: &Path) -> Result<String, Failure> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(Failure::Io(e.to_string())),
    }
}

/// Write the whole file at once, so KWin never reads half of it. A file that is
/// a symbolic link (a dotfiles folder) is written where the link points, so the
/// link stays; the file keeps its permissions.
fn write_atomic(path: &Path, text: &str) -> Result<(), Failure> {
    let write = || -> io::Result<()> {
        let target = match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => fs::canonicalize(path)?,
            _ => path.to_path_buf(),
        };
        if let Some(dir) = target.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut tmp = target.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        fs::write(&tmp, text)?;
        if let Ok(meta) = fs::metadata(&target) {
            fs::set_permissions(&tmp, meta.permissions())?;
        }
        fs::rename(&tmp, &target)
    };
    write().map_err(|e| Failure::Io(e.to_string()))
}

/// KDE Plasma, going by the session's desktop name.
pub fn is_kde(env: Env) -> bool {
    let named = |name: &str| env(name).and_then(|value| value.into_string().ok());
    named("XDG_CURRENT_DESKTOP")
        .is_some_and(|desktops| desktops.split(':').any(|d| d.eq_ignore_ascii_case("KDE")))
        || named("KDE_FULL_SESSION").is_some_and(|value| value == "true")
}

fn absolute(env: Env, name: &str) -> Option<PathBuf> {
    env(name).map(PathBuf::from).filter(|p| p.is_absolute())
}

/// `$XDG_CONFIG_HOME/kwinrc`, else `~/.config/kwinrc`.
pub fn kwinrc_path(env: Env) -> Option<PathBuf> {
    absolute(env, "XDG_CONFIG_HOME")
        .or_else(|| absolute(env, "HOME").map(|home| home.join(".config")))
        .map(|dir| dir.join("kwinrc"))
}

/// easime's `.desktop` file: the user's applications first (a development
/// install), then the system's.
pub fn find_desktop_file(env: Env) -> Option<PathBuf> {
    let user = absolute(env, "XDG_DATA_HOME")
        .or_else(|| absolute(env, "HOME").map(|home| home.join(".local").join("share")));
    let system = env("XDG_DATA_DIRS")
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    user.into_iter()
        .chain(system.split(':').map(PathBuf::from))
        .map(|dir| dir.join("applications").join(DESKTOP_FILE))
        .find(|path| path.is_file())
}

/// True if a process named `name` owned by the current user exists, going by
/// `<proc>/<pid>/comm`.
pub fn process_named(proc: &Path, name: &str) -> bool {
    let Ok(entries) = fs::read_dir(proc) else {
        return false;
    };
    #[cfg(unix)]
    let self_uid = {
        use std::os::unix::fs::MetadataExt;
        fs::metadata(proc.join("self")).ok().map(|m| m.uid())
    };
    entries.flatten().any(|entry| {
        if !entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|b| b.is_ascii_digit())
        {
            return false;
        }
        #[cfg(unix)]
        if let Some(uid) = self_uid {
            use std::os::unix::fs::MetadataExt;
            if fs::metadata(entry.path()).ok().map(|m| m.uid()) != Some(uid) {
                return false;
            }
        }
        fs::read_to_string(entry.path().join("comm")).is_ok_and(|comm| comm.trim_end() == name)
    })
}

/// The value of `key` in `[group]` of an INI file as KDE writes them.
pub fn ini_get(text: &str, group: &str, key: &str) -> Option<String> {
    let mut in_group = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = group_header(trimmed) {
            in_group = name == group;
        } else if in_group && let Some(value) = key_value(trimmed, key) {
            return Some(value.to_owned());
        }
    }
    None
}

/// `text` with `key` in `[group]` set to `value` (or removed for `None`). Every
/// other line stays exactly as it was, comments and other groups included.
pub fn ini_set(text: &str, group: &str, key: &str, value: Option<&str>) -> String {
    let new_line = value.map(|value| format!("{key}={value}"));
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut in_group = false;
    let mut group_end = None; // index after the group's last non-empty line
    let mut at = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(name) = group_header(trimmed) {
            in_group = name == group;
            if in_group {
                group_end = Some(i + 1);
            }
        } else if in_group {
            if key_value(trimmed, key).is_some() {
                at = Some(i);
                break;
            }
            if !trimmed.is_empty() {
                group_end = Some(i + 1);
            }
        }
    }
    match (at, new_line, group_end) {
        (Some(i), Some(_), _) => {
            // Keep the key as it was written, markers included.
            let name = lines[i]
                .split_once('=')
                .map(|(name, _)| name.trim().to_owned())
                .unwrap_or_else(|| key.to_owned());
            lines[i] = format!("{name}={}", value.unwrap_or_default());
        }
        (Some(i), None, _) => {
            lines.remove(i);
        }
        (None, Some(line), Some(end)) => lines.insert(end, line),
        (None, Some(line), None) => {
            if lines.last().is_some_and(|last| !last.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(format!("[{group}]"));
            lines.push(line);
        }
        (None, None, _) => {}
    }
    let mut out = lines.join("\n");
    if !lines.is_empty() {
        out.push('\n');
    }
    out
}

/// `[name]` → `name` (a plain group, not a nested `[a][b]` one).
fn group_header(line: &str) -> Option<&str> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    (!inner.contains('[') && !inner.contains(']')).then_some(inner)
}

/// `key=value` → `value`, for exactly this key. KDE may add option markers to
/// the key (`InputMethod[$e]=...`: `$e` expands variables such as `$HOME` in
/// the value; System Settings writes the input method this way), which are part
/// of the line but not of the key's name.
fn key_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let (name, value) = line.split_once('=')?;
    (key_name(name.trim()) == key).then_some(value.trim())
}

/// The key's name without its option markers: `InputMethod[$e]` → `InputMethod`.
/// (A localized key, `Name[de]`, is another key and keeps its suffix; only `$`
/// markers are options.)
fn key_name(name: &str) -> &str {
    match name.find("[$") {
        Some(at) if name.ends_with(']') => &name[..at],
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Dir(PathBuf);

    impl Dir {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "easime-activation-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    const OTHER: &str = "/usr/share/applications/org.kde.plasma.keyboard.desktop";

    /// A KDE session with easime installed, and the daemon not running.
    fn setup(dir: &Dir, kwinrc: Option<&str>) -> Activation {
        let desktop = dir.path("easime.desktop");
        fs::write(&desktop, "[Desktop Entry]\n").unwrap();
        if let Some(text) = kwinrc {
            fs::write(dir.path("kwinrc"), text).unwrap();
        }
        Activation::new(
            true,
            Some(dir.path("kwinrc")),
            Some(desktop),
            Some(dir.path("backup.toml")),
            Box::new(|| false),
        )
    }

    fn kwinrc_text(dir: &Dir) -> String {
        fs::read_to_string(dir.path("kwinrc")).unwrap()
    }

    #[test]
    fn ini_values_are_read_from_their_group_only() {
        let text =
            "[Compositing]\nInputMethod=wrong\n\n[Wayland]\nInputMethod = /a/b.desktop\nOther=1\n";
        assert_eq!(
            ini_get(text, "Wayland", "InputMethod"),
            Some("/a/b.desktop".into())
        );
        assert_eq!(
            ini_get(text, "Compositing", "InputMethod"),
            Some("wrong".into())
        );
        assert_eq!(ini_get(text, "Wayland", "Missing"), None);
        assert_eq!(ini_get(text, "Nope", "InputMethod"), None);
        assert_eq!(ini_get("", "Wayland", "InputMethod"), None);
    }

    #[test]
    fn setting_a_value_changes_nothing_else() {
        let text = "# my kwin\n[Compositing]\nBackend=OpenGL\n\n[Wayland]\n# keyboard\nInputMethod=/old.desktop\nKeyRepeat=1\n\n[Xwayland]\nScale=2\n";
        let set = ini_set(text, "Wayland", "InputMethod", Some("/new.desktop"));
        assert_eq!(set, text.replace("/old.desktop", "/new.desktop"));
    }

    #[test]
    fn a_new_key_goes_at_the_end_of_its_group() {
        let text = "[Wayland]\nKeyRepeat=1\n\n[Xwayland]\nScale=2\n";
        assert_eq!(
            ini_set(text, "Wayland", "InputMethod", Some("/x.desktop")),
            "[Wayland]\nKeyRepeat=1\nInputMethod=/x.desktop\n\n[Xwayland]\nScale=2\n"
        );
    }

    #[test]
    fn a_missing_group_or_file_is_created() {
        assert_eq!(
            ini_set(
                "[Compositing]\nBackend=OpenGL\n",
                "Wayland",
                "InputMethod",
                Some("/x")
            ),
            "[Compositing]\nBackend=OpenGL\n\n[Wayland]\nInputMethod=/x\n"
        );
        assert_eq!(
            ini_set("", "Wayland", "InputMethod", Some("/x")),
            "[Wayland]\nInputMethod=/x\n"
        );
    }

    #[test]
    fn a_value_can_be_removed() {
        let text = "[Wayland]\nInputMethod=/x\nKeyRepeat=1\n";
        assert_eq!(
            ini_set(text, "Wayland", "InputMethod", None),
            "[Wayland]\nKeyRepeat=1\n"
        );
        assert_eq!(ini_set(text, "Wayland", "Absent", None), text);
        assert_eq!(ini_set("", "Wayland", "InputMethod", None), "");
    }

    #[test]
    fn names_come_from_the_desktop_file() {
        assert_eq!(name_of(OTHER), "org.kde.plasma.keyboard");
        assert_eq!(name_of("/x/easime.desktop"), "easime");
        assert_eq!(name_of("plain"), "plain");
    }

    #[test]
    fn plasma_is_told_by_the_desktop_name() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| OsString::from(v))
            }
        };
        assert!(is_kde(&env(&[("XDG_CURRENT_DESKTOP", "KDE")])));
        assert!(is_kde(&env(&[("XDG_CURRENT_DESKTOP", "ubuntu:KDE")])));
        assert!(is_kde(&env(&[("KDE_FULL_SESSION", "true")])));
        assert!(!is_kde(&env(&[("XDG_CURRENT_DESKTOP", "GNOME")])));
        assert!(!is_kde(&env(&[])));
    }

    #[test]
    fn another_input_method_can_be_replaced_and_restored() {
        let dir = Dir::new();
        let original = format!("[Wayland]\nInputMethod={OTHER}\nKeyRepeat=1\n");
        let activation = setup(&dir, Some(&original));
        assert_eq!(
            activation.state(),
            State::Other {
                current: Some("org.kde.plasma.keyboard".into())
            }
        );
        let plan = activation.plan().unwrap();
        assert_eq!(plan.from.as_deref(), Some("org.kde.plasma.keyboard"));
        assert_eq!(plan.to, dir.path("easime.desktop"));
        assert_eq!(plan.file, dir.path("kwinrc"));

        activation.activate().unwrap();
        assert_eq!(
            kwinrc_text(&dir),
            format!(
                "[Wayland]\nInputMethod={}\nKeyRepeat=1\n",
                dir.path("easime.desktop").display()
            )
        );
        assert_eq!(
            activation.state(),
            State::Selected {
                running: false,
                can_undo: true
            }
        );
        assert_eq!(
            activation.undo_target(),
            Some(Some("org.kde.plasma.keyboard".into()))
        );

        activation.undo().unwrap();
        assert_eq!(kwinrc_text(&dir), original, "exactly as it was");
        assert!(!dir.path("backup.toml").exists());
        assert_eq!(activation.undo_target(), None);
    }

    #[test]
    fn with_no_input_method_set_undo_removes_the_key_again() {
        let dir = Dir::new();
        let original = "[Wayland]\nKeyRepeat=1\n";
        let activation = setup(&dir, Some(original));
        assert_eq!(activation.state(), State::Other { current: None });
        activation.activate().unwrap();
        assert!(ini_get(&kwinrc_text(&dir), GROUP, KEY).is_some());
        assert_eq!(activation.undo_target(), Some(None));
        activation.undo().unwrap();
        assert_eq!(kwinrc_text(&dir), original);
    }

    #[test]
    fn a_missing_kwinrc_is_created_and_removed_content_wise() {
        let dir = Dir::new();
        let activation = setup(&dir, None);
        assert_eq!(activation.state(), State::Other { current: None });
        activation.activate().unwrap();
        assert!(kwinrc_text(&dir).starts_with("[Wayland]\nInputMethod="));
        activation.undo().unwrap();
        assert_eq!(kwinrc_text(&dir), "[Wayland]\n");
    }

    #[test]
    fn an_undo_never_overwrites_a_choice_made_since() {
        let dir = Dir::new();
        let activation = setup(&dir, Some(&format!("[Wayland]\nInputMethod={OTHER}\n")));
        activation.activate().unwrap();
        // The user picks yet another input method in System Settings.
        let chosen = "[Wayland]\nInputMethod=/usr/share/applications/fcitx5.desktop\n";
        fs::write(dir.path("kwinrc"), chosen).unwrap();
        assert_eq!(activation.undo_target(), None);
        assert_eq!(activation.undo(), Err(Failure::Changed));
        assert_eq!(kwinrc_text(&dir), chosen, "left alone");
    }

    #[test]
    fn nothing_is_possible_when_easime_is_already_selected() {
        let dir = Dir::new();
        let activation = setup(
            &dir,
            Some(&format!(
                "[Wayland]\nInputMethod={}\n",
                dir.path("easime.desktop").display()
            )),
        );
        assert!(matches!(
            activation.state(),
            State::Selected {
                can_undo: false,
                ..
            }
        ));
        assert_eq!(activation.plan(), None);
        assert_eq!(activation.activate(), Err(Failure::NotPossible));
        // Selected by hand: there is no backup, so there is nothing to undo.
        assert_eq!(activation.undo(), Err(Failure::NotPossible));
    }

    #[test]
    fn the_daemon_running_is_reported_once_it_is_selected() {
        let dir = Dir::new();
        let desktop = dir.path("easime.desktop");
        fs::write(&desktop, "").unwrap();
        fs::write(
            dir.path("kwinrc"),
            format!("[Wayland]\nInputMethod={}\n", desktop.display()),
        )
        .unwrap();
        let activation = Activation::new(
            true,
            Some(dir.path("kwinrc")),
            Some(desktop),
            Some(dir.path("backup.toml")),
            Box::new(|| true),
        );
        assert_eq!(
            activation.state(),
            State::Selected {
                running: true,
                can_undo: false
            }
        );
    }

    #[test]
    fn an_easime_file_elsewhere_counts_as_easime() {
        // Selected from a development install, found in the system one.
        assert!(is_easime(
            "/home/u/.local/share/applications/easime.desktop",
            Path::new("/usr/share/applications/easime.desktop")
        ));
        assert!(!is_easime(
            OTHER,
            Path::new("/usr/share/applications/easime.desktop")
        ));
    }

    #[test]
    fn not_kde_and_not_installed_offer_nothing() {
        let dir = Dir::new();
        let mut activation = setup(&dir, None);
        activation.kde = false;
        assert_eq!(activation.state(), State::Unsupported);
        assert_eq!(activation.plan(), None);
        let mut activation = setup(&dir, None);
        activation.desktop_file = None;
        assert_eq!(activation.state(), State::NotInstalled);
        assert_eq!(activation.activate(), Err(Failure::NotPossible));
    }

    #[test]
    fn a_choice_made_in_system_settings_with_a_marker_is_understood() {
        // System Settings writes the key with the `$e` option marker.
        let dir = Dir::new();
        let other = format!("[Wayland]\nInputMethod[$e]={OTHER}\n");
        let activation = setup(&dir, Some(&other));
        assert_eq!(
            activation.state(),
            State::Other {
                current: Some("org.kde.plasma.keyboard".into())
            },
            "the input method chosen there is seen"
        );
        activation.activate().unwrap();
        let after = kwinrc_text(&dir);
        assert_eq!(
            after.matches("InputMethod").count(),
            1,
            "the line is changed, not doubled: {after}"
        );
        assert!(
            after.contains("InputMethod[$e]="),
            "the marker stays: {after}"
        );
        assert!(matches!(activation.state(), State::Selected { .. }));
        activation.undo().unwrap();
        assert_eq!(
            kwinrc_text(&dir),
            other,
            "undo gives back exactly the old file"
        );

        // easime chosen in System Settings: already selected, nothing to offer.
        let dir = Dir::new();
        let activation = setup(&dir, None);
        let mine = format!(
            "[Wayland]\nInputMethod[$e]={}\n",
            dir.path("easime.desktop").display()
        );
        fs::write(dir.path("kwinrc"), mine).unwrap();
        assert!(matches!(activation.state(), State::Selected { .. }));
        assert_eq!(activation.plan(), None);
    }

    #[test]
    fn a_localized_key_is_not_the_key() {
        let text = "[Wayland]\nInputMethod[de]=x\n";
        assert_eq!(ini_get(text, "Wayland", "InputMethod"), None);
        assert_eq!(key_name("InputMethod[$e]"), "InputMethod");
        assert_eq!(key_name("InputMethod[$i]"), "InputMethod");
        assert_eq!(key_name("Name[de]"), "Name[de]");
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_kwinrc_stays_a_link_and_keeps_its_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = Dir::new();
        let real = dir.path("dotfiles-kwinrc");
        fs::write(&real, format!("[Wayland]\nInputMethod={OTHER}\n")).unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        let activation = setup(&dir, None);
        std::os::unix::fs::symlink(&real, dir.path("kwinrc")).unwrap();
        activation.activate().unwrap();
        assert!(
            fs::symlink_metadata(dir.path("kwinrc"))
                .unwrap()
                .file_type()
                .is_symlink(),
            "still a link"
        );
        assert!(
            fs::read_to_string(&real)
                .unwrap()
                .contains("easime.desktop")
        );
        assert_eq!(
            fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn a_failed_write_changes_nothing() {
        let dir = Dir::new();
        let activation = setup(&dir, None);
        // A directory where kwinrc should be: reading and writing both fail.
        fs::create_dir_all(dir.path("kwinrc")).unwrap();
        assert!(matches!(activation.activate(), Err(Failure::Io(_))));
        assert!(
            !dir.path("backup.toml").exists(),
            "no backup for a change that did not happen"
        );
    }

    #[test]
    fn the_desktop_file_is_found_in_the_user_and_system_folders() {
        let dir = Dir::new();
        let user = dir.path("user");
        let system = dir.path("system");
        let home = dir.path("home");
        for base in [&user, &system] {
            fs::create_dir_all(base.join("applications")).unwrap();
        }
        let env = |data_home: Option<PathBuf>, data_dirs: String| {
            let home = home.clone();
            move |name: &str| match name {
                "XDG_DATA_HOME" => data_home.clone().map(PathBuf::into_os_string),
                "XDG_DATA_DIRS" => Some(OsString::from(&data_dirs)),
                "HOME" => Some(home.clone().into_os_string()),
                _ => None,
            }
        };
        let dirs = system.to_string_lossy().into_owned();
        assert_eq!(
            find_desktop_file(&env(Some(user.clone()), dirs.clone())),
            None
        );
        fs::write(system.join("applications").join(DESKTOP_FILE), "").unwrap();
        assert_eq!(
            find_desktop_file(&env(Some(user.clone()), dirs.clone())),
            Some(system.join("applications").join(DESKTOP_FILE))
        );
        fs::write(user.join("applications").join(DESKTOP_FILE), "").unwrap();
        assert_eq!(
            find_desktop_file(&env(Some(user.clone()), dirs)),
            Some(user.join("applications").join(DESKTOP_FILE)),
            "a development install in the user's folder comes first"
        );
    }

    #[test]
    fn kwinrc_follows_the_config_home() {
        let dir = Dir::new();
        let (config, home) = (dir.path("config"), dir.path("home"));
        let env = |config: Option<PathBuf>| {
            let home = home.clone();
            move |name: &str| match name {
                "XDG_CONFIG_HOME" => config.clone().map(PathBuf::into_os_string),
                "HOME" => Some(home.clone().into_os_string()),
                _ => None,
            }
        };
        assert_eq!(
            kwinrc_path(&env(Some(config.clone()))),
            Some(config.join("kwinrc"))
        );
        assert_eq!(
            kwinrc_path(&env(None)),
            Some(home.join(".config").join("kwinrc"))
        );
    }

    #[test]
    fn a_process_is_found_by_its_name() {
        let dir = Dir::new();
        for (pid, name) in [
            ("1", "systemd\n"),
            ("42", "easime\n"),
            ("43", "easime-settings\n"),
        ] {
            fs::create_dir_all(dir.path(pid)).unwrap();
            fs::write(dir.path(pid).join("comm"), name).unwrap();
        }
        fs::create_dir_all(dir.path("self")).unwrap();
        fs::write(dir.path("self").join("comm"), "easime-x\n").unwrap();
        assert!(process_named(&dir.0, "easime"));
        assert!(
            !process_named(&dir.0, "easime-x"),
            "only numbered entries are processes"
        );
        assert!(process_named(&dir.0, "easime-settings"));
        assert!(!process_named(&dir.0, "fcitx5"));
        assert!(!process_named(&dir.path("missing"), "easime"));
    }
}
