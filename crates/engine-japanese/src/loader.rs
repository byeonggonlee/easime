//! Finding, loading and combining dictionaries.
//!
//! The daemon calls [`load_default`] once at startup (ideally off the UI
//! thread, the big file takes about 0.1 s to validate) and hands the
//! resulting [`LayeredDictionary`] to
//! [`JapaneseEngine::with_dictionary`](crate::JapaneseEngine::with_dictionary).
//!
//! Directories searched, in this order (earlier entries win): first the user
//! directory, then the system directory. Where those are depends on the
//! operating system ([`ime_core::Os`]):
//!
//! | OS | user directory | system directory |
//! |---|---|---|
//! | Linux | `$XDG_DATA_HOME/easime/dictionary` (default `~/.local/share/easime/dictionary`) | `/usr/share/easime/dictionary` |
//! | macOS | `~/Library/Application Support/easime/dictionary` | `/Library/Application Support/easime/dictionary` |
//! | Windows | `%APPDATA%\easime\dictionary` | `%ProgramW6432%\easime\dictionary` (else `%ProgramFiles%`) |
//!
//! Only Linux is in use today. The macOS and Windows rows are the planned
//! places (the Windows one is the folder the MSI installs into) and are
//! checked by unit tests on every OS, but no installer puts a dictionary
//! there yet. See `docs/platforms.md`.
//!
//! In each directory `*.tsv` files (the [`MemoryDictionary`] text format)
//! and `*.bin` files (the [`FileDictionary`] binary format) are loaded in
//! file name order. Other files are ignored. A directory that does not
//! exist is simply skipped; a file that cannot be loaded is reported in
//! [`LoadReport::failed`] and does not stop the others.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::dictionary::{Dictionary, MemoryDictionary};
use crate::file_dict::FileDictionary;
use ime_core::Os;

/// Where Linux distribution packages install dictionaries.
pub const SYSTEM_DICTIONARY_DIR: &str = "/usr/share/easime/dictionary";

/// Several dictionaries searched together. Candidates of earlier layers come
/// first; a candidate already offered by an earlier layer is not repeated.
#[derive(Default)]
pub struct LayeredDictionary {
    layers: Vec<Box<dyn Dictionary>>,
}

impl LayeredDictionary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a layer after the existing ones (lower priority).
    pub fn push(&mut self, layer: Box<dyn Dictionary>) {
        self.layers.push(layer);
    }

    /// Builder style [`push`](Self::push).
    pub fn with_layer(mut self, layer: Box<dyn Dictionary>) -> Self {
        self.push(layer);
        self
    }

    /// Number of layers.
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }
}

impl std::fmt::Debug for LayeredDictionary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayeredDictionary")
            .field("layers", &self.layers.len())
            .finish()
    }
}

impl Dictionary for LayeredDictionary {
    fn lookup(&self, reading: &str) -> Vec<String> {
        let mut merged: Vec<String> = Vec::new();
        for layer in &self.layers {
            for candidate in layer.lookup(reading) {
                if !merged.contains(&candidate) {
                    merged.push(candidate);
                }
            }
        }
        merged
    }
}

/// Environment variable lookup, passed in so tests do not touch the real one.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The value of an environment variable if it is an absolute path.
fn absolute(env: Env, name: &str) -> Option<PathBuf> {
    env(name).map(PathBuf::from).filter(|p| p.is_absolute())
}

fn under_easime(base: PathBuf) -> PathBuf {
    base.join("easime").join("dictionary")
}

/// The user dictionary directory on `os`, or `None` if its base directory is
/// not known. Relative paths in environment variables are ignored (the XDG
/// specification requires that for `XDG_DATA_HOME`; the others follow it).
pub fn user_dictionary_dir_for(os: Os, env: Env) -> Option<PathBuf> {
    let base = match os {
        Os::Linux => absolute(env, "XDG_DATA_HOME")
            .or_else(|| absolute(env, "HOME").map(|home| home.join(".local").join("share")))?,
        Os::MacOs => absolute(env, "HOME")?
            .join("Library")
            .join("Application Support"),
        Os::Windows => absolute(env, "APPDATA")?,
    };
    Some(under_easime(base))
}

/// The system-wide dictionary directory on `os`.
///
/// On Windows `ProgramW6432` is the 64-bit Program Files even for a 32-bit
/// process (a text service DLL is loaded into 32-bit applications too);
/// `ProgramFiles` is the fallback.
pub fn system_dictionary_dir_for(os: Os, env: Env) -> Option<PathBuf> {
    match os {
        Os::Linux => Some(PathBuf::from(SYSTEM_DICTIONARY_DIR)),
        Os::MacOs => Some(under_easime(
            PathBuf::from("/Library").join("Application Support"),
        )),
        Os::Windows => absolute(env, "ProgramW6432")
            .or_else(|| absolute(env, "ProgramFiles"))
            .map(under_easime),
    }
}

/// User directory first, then the system directory, for `os`.
pub fn dictionary_dirs_for(os: Os, env: Env) -> Vec<PathBuf> {
    user_dictionary_dir_for(os, env)
        .into_iter()
        .chain(system_dictionary_dir_for(os, env))
        .collect()
}

/// The user dictionary directory on this OS, from the real environment.
pub fn user_dictionary_dir() -> Option<PathBuf> {
    user_dictionary_dir_for(Os::current(), &|name| std::env::var_os(name))
}

/// User directory first, then the system directory, on this OS.
pub fn default_dictionary_dirs() -> Vec<PathBuf> {
    dictionary_dirs_for(Os::current(), &|name| std::env::var_os(name))
}

/// Result of [`load_dictionaries`].
#[derive(Debug, Default)]
pub struct LoadReport {
    /// All loaded files as layers, in priority order.
    pub dictionary: LayeredDictionary,
    /// Files that were loaded, in priority order.
    pub loaded: Vec<PathBuf>,
    /// Files that could not be loaded, with the reason.
    pub failed: Vec<(PathBuf, String)>,
    /// Malformed lines skipped in `.tsv` files.
    pub skipped_lines: usize,
}

/// How binary (`.bin`) dictionaries are opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Access {
    /// Read into the process's memory (one process uses the dictionary: the
    /// Linux daemon).
    #[default]
    Read,
    /// Mapped from the file, so every process that opens it shares the same
    /// memory (the Windows text service runs inside every application). See
    /// [`FileDictionary::map`].
    Map,
}

/// Load every `.tsv` and `.bin` dictionary in `dirs`, directories in the
/// order given (put the user directory first), files by name.
pub fn load_dictionaries(dirs: &[PathBuf]) -> LoadReport {
    load_dictionaries_with(dirs, Access::Read)
}

/// [`load_dictionaries`], opening `.bin` files as `access` says.
pub fn load_dictionaries_with(dirs: &[PathBuf], access: Access) -> LoadReport {
    let mut report = LoadReport::default();
    for dir in dirs {
        let mut files: Vec<PathBuf> = match std::fs::read_dir(dir) {
            Ok(entries) => entries.filter_map(Result::ok).map(|e| e.path()).collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                report.failed.push((dir.clone(), e.to_string()));
                continue;
            }
        };
        files.sort();
        for path in files {
            let extension = path
                .extension()
                .and_then(OsStr::to_str)
                .map(str::to_ascii_lowercase);
            let layer: Result<Box<dyn Dictionary>, String> = match extension.as_deref() {
                Some("tsv") => load_text(&path, &mut report.skipped_lines),
                Some("bin") => match access {
                    Access::Read => FileDictionary::open(&path),
                    Access::Map => FileDictionary::map(&path),
                }
                .map(|d| Box::new(d) as Box<dyn Dictionary>)
                .map_err(|e| e.to_string()),
                _ => continue,
            };
            match layer {
                Ok(layer) => {
                    report.dictionary.push(layer);
                    report.loaded.push(path);
                }
                Err(why) => report.failed.push((path, why)),
            }
        }
    }
    report
}

fn load_text(path: &Path, skipped: &mut usize) -> Result<Box<dyn Dictionary>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let (dictionary, skipped_here) = MemoryDictionary::parse(&text);
    *skipped += skipped_here;
    Ok(Box::new(dictionary))
}

/// [`load_dictionaries`] on [`default_dictionary_dirs`].
pub fn load_default() -> LoadReport {
    load_dictionaries(&default_dictionary_dirs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_dict::encode;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "easime-loader-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn write(&self, name: &str, content: impl AsRef<[u8]>) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, content).unwrap();
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn bin(entries: &[(&str, &[&str])]) -> Vec<u8> {
        let owned: Vec<(String, Vec<String>)> = entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.iter().map(|s| s.to_string()).collect()))
            .collect();
        encode(
            "test",
            owned.iter().map(|(k, v)| (k.as_str(), v.as_slice())),
        )
        .unwrap()
    }

    fn memory(text: &str) -> Box<dyn Dictionary> {
        Box::new(MemoryDictionary::parse(text).0)
    }

    #[test]
    fn layers_merge_in_order_without_duplicates() {
        let d = LayeredDictionary::new()
            .with_layer(memory("かみ\t神,紙\n"))
            .with_layer(memory("かみ\t髪,紙\nいぬ\t犬\n"));
        assert_eq!(d.layer_count(), 2);
        assert_eq!(d.lookup("かみ"), ["神", "紙", "髪"]);
        assert_eq!(d.lookup("いぬ"), ["犬"]);
        assert!(d.lookup("ねこ").is_empty());
    }

    #[test]
    fn empty_layered_dictionary_knows_nothing() {
        let d = LayeredDictionary::new();
        assert!(d.is_empty());
        assert!(d.lookup("かみ").is_empty());
    }

    #[test]
    fn layers_can_be_file_dictionaries() {
        let file = FileDictionary::from_bytes(bin(&[("かみ", &["紙", "神"])])).unwrap();
        let d = LayeredDictionary::new()
            .with_layer(memory("かみ\t上\n"))
            .with_layer(Box::new(file));
        assert_eq!(d.lookup("かみ"), ["上", "紙", "神"]);
    }

    /// An environment of the given variables.
    fn env_of(pairs: &[(&str, &Path)]) -> impl Fn(&str) -> Option<OsString> + use<> {
        let pairs: Vec<(String, OsString)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.as_os_str().to_owned()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
        }
    }

    /// Absolute paths are platform specific ("/data" has no drive letter, so
    /// Windows does not call it absolute): build them from the temporary
    /// directory, which is absolute everywhere.
    fn abs(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    fn easime(base: &Path) -> PathBuf {
        base.join("easime").join("dictionary")
    }

    #[test]
    fn linux_follows_the_xdg_rules() {
        let (data, home) = (abs("data"), abs("home"));
        let from_home = easime(&home.join(".local").join("share"));
        let dir = |pairs: &[(&str, &Path)]| user_dictionary_dir_for(Os::Linux, &env_of(pairs));

        assert_eq!(
            dir(&[("XDG_DATA_HOME", &data), ("HOME", &home)]),
            Some(easime(&data))
        );
        assert_eq!(dir(&[("HOME", &home)]), Some(from_home.clone()));
        // A relative XDG_DATA_HOME is ignored, and so is an empty one.
        assert_eq!(
            dir(&[("XDG_DATA_HOME", Path::new("relative")), ("HOME", &home)]),
            Some(from_home.clone())
        );
        assert_eq!(
            dir(&[("XDG_DATA_HOME", Path::new("")), ("HOME", &home)]),
            Some(from_home)
        );
        assert_eq!(dir(&[]), None);
        assert_eq!(dir(&[("HOME", Path::new(""))]), None);
        assert_eq!(
            system_dictionary_dir_for(Os::Linux, &env_of(&[])),
            Some(PathBuf::from(SYSTEM_DICTIONARY_DIR))
        );
    }

    #[test]
    fn macos_keeps_dictionaries_in_application_support() {
        let home = abs("home");
        let user = user_dictionary_dir_for(Os::MacOs, &env_of(&[("HOME", &home)]));
        assert_eq!(
            user,
            Some(easime(&home.join("Library").join("Application Support")))
        );
        assert_eq!(user_dictionary_dir_for(Os::MacOs, &env_of(&[])), None);
        // XDG variables mean nothing on macOS.
        assert_eq!(
            user_dictionary_dir_for(Os::MacOs, &env_of(&[("XDG_DATA_HOME", &abs("data"))])),
            None
        );
        assert_eq!(
            system_dictionary_dir_for(Os::MacOs, &env_of(&[])),
            Some(easime(
                &PathBuf::from("/Library").join("Application Support")
            ))
        );
    }

    #[test]
    fn windows_uses_appdata_and_program_files() {
        let (appdata, w6432, files) = (abs("appdata"), abs("program-w6432"), abs("program-files"));
        assert_eq!(
            user_dictionary_dir_for(Os::Windows, &env_of(&[("APPDATA", &appdata)])),
            Some(easime(&appdata))
        );
        // HOME and XDG variables mean nothing on Windows.
        assert_eq!(
            user_dictionary_dir_for(Os::Windows, &env_of(&[("HOME", &abs("home"))])),
            None
        );
        let system =
            |pairs: &[(&str, &Path)]| system_dictionary_dir_for(Os::Windows, &env_of(pairs));
        // The 64-bit Program Files wins, also for a 32-bit process.
        assert_eq!(
            system(&[("ProgramW6432", &w6432), ("ProgramFiles", &files)]),
            Some(easime(&w6432))
        );
        assert_eq!(system(&[("ProgramFiles", &files)]), Some(easime(&files)));
        assert_eq!(system(&[("ProgramFiles", Path::new("relative"))]), None);
        assert_eq!(system(&[]), None);
    }

    #[test]
    fn the_user_directory_comes_before_the_system_directory() {
        let home = abs("home");
        let dirs = dictionary_dirs_for(Os::Linux, &env_of(&[("HOME", &home)]));
        assert_eq!(
            dirs,
            [
                easime(&home.join(".local").join("share")),
                PathBuf::from(SYSTEM_DICTIONARY_DIR)
            ]
        );
        // Without a user directory only the system one is searched.
        assert_eq!(
            dictionary_dirs_for(Os::Linux, &env_of(&[])),
            [PathBuf::from(SYSTEM_DICTIONARY_DIR)]
        );
        assert!(dictionary_dirs_for(Os::Windows, &env_of(&[])).is_empty());
    }

    #[test]
    fn this_os_uses_its_own_rules() {
        let real = |name: &str| std::env::var_os(name);
        assert_eq!(
            default_dictionary_dirs(),
            dictionary_dirs_for(Os::current(), &real)
        );
        assert_eq!(
            user_dictionary_dir(),
            user_dictionary_dir_for(Os::current(), &real)
        );
    }

    #[test]
    fn loads_tsv_and_bin_files_by_name_and_ignores_the_rest() {
        let t = TempDir::new();
        t.write("b.bin", bin(&[("かみ", &["紙"])]));
        t.write("a.tsv", "かみ\t神\n");
        t.write("c.TSV", "かみ\t髪\n");
        t.write("notes.txt", "かみ\t無視\n");
        t.write("noext", "かみ\t無視\n");
        let report = load_dictionaries(std::slice::from_ref(&t.0));
        assert!(report.failed.is_empty(), "{:?}", report.failed);
        assert_eq!(report.loaded.len(), 3);
        assert_eq!(report.dictionary.lookup("かみ"), ["神", "紙", "髪"]);
    }

    #[test]
    fn mapped_binary_files_give_the_same_words() {
        let t = TempDir::new();
        t.write("b.bin", bin(&[("かみ", &["紙"])]));
        t.write("a.tsv", "かみ\t神\n");
        t.write("broken.bin", "not a dictionary");
        let read = load_dictionaries(std::slice::from_ref(&t.0));
        let mapped = load_dictionaries_with(std::slice::from_ref(&t.0), Access::Map);
        assert_eq!(mapped.loaded, read.loaded);
        assert_eq!(mapped.failed.len(), 1, "{:?}", mapped.failed);
        assert_eq!(mapped.dictionary.lookup("かみ"), ["神", "紙"]);
        assert_eq!(
            mapped.dictionary.lookup("かみ"),
            read.dictionary.lookup("かみ")
        );
    }

    #[test]
    fn user_directory_entries_come_first() {
        let user = TempDir::new();
        let system = TempDir::new();
        user.write("mine.tsv", "かみ\t私の神\n");
        system.write("mozc.bin", bin(&[("かみ", &["神", "紙"])]));
        let report = load_dictionaries(&[user.0.clone(), system.0.clone()]);
        assert_eq!(report.dictionary.lookup("かみ"), ["私の神", "神", "紙"]);
        // The other order flips the priority.
        let report = load_dictionaries(&[system.0.clone(), user.0.clone()]);
        assert_eq!(report.dictionary.lookup("かみ"), ["神", "紙", "私の神"]);
    }

    #[test]
    fn missing_directories_are_skipped_silently() {
        let t = TempDir::new();
        t.write("a.tsv", "いぬ\t犬\n");
        let report = load_dictionaries(&[t.0.join("does-not-exist"), t.0.clone()]);
        assert!(report.failed.is_empty());
        assert_eq!(report.dictionary.lookup("いぬ"), ["犬"]);
        assert!(load_dictionaries(&[]).dictionary.is_empty());
    }

    #[test]
    fn broken_files_are_reported_and_do_not_stop_the_rest() {
        let t = TempDir::new();
        let good = bin(&[("かみ", &["紙"])]);
        t.write("a-truncated.bin", &good[..good.len() - 3]);
        t.write(
            "b-garbage.bin",
            "this is not a dictionary at all, not even close",
        );
        t.write("c-not-utf8.tsv", [0xffu8, 0xfe, b'\t', b'x']);
        t.write("d-good.bin", &good);
        t.write("e.tsv", "bad line\nいぬ\t犬\n");
        let report = load_dictionaries(std::slice::from_ref(&t.0));
        assert_eq!(report.failed.len(), 3, "{:?}", report.failed);
        assert_eq!(report.loaded.len(), 2);
        assert_eq!(report.skipped_lines, 1);
        assert_eq!(report.dictionary.lookup("かみ"), ["紙"]);
        assert_eq!(report.dictionary.lookup("いぬ"), ["犬"]);
        assert!(report.failed[0].0.ends_with("a-truncated.bin"));
    }

    #[test]
    fn a_directory_named_like_a_dictionary_is_a_failure_not_a_crash() {
        let t = TempDir::new();
        std::fs::create_dir(t.0.join("odd.bin")).unwrap();
        let report = load_dictionaries(std::slice::from_ref(&t.0));
        assert_eq!(report.failed.len(), 1);
        assert!(report.loaded.is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn a_file_where_a_directory_is_expected_is_a_failure() {
        let t = TempDir::new();
        let file = t.write("plain", "x");
        let report = load_dictionaries(&[file]);
        assert_eq!(report.failed.len(), 1);
    }
}
