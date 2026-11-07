//! Finding, loading and combining dictionaries.
//!
//! The daemon calls [`load_default`] once at startup (ideally off the UI
//! thread, the big file takes about 0.1 s to validate) and hands the
//! resulting [`LayeredDictionary`] to
//! [`JapaneseEngine::with_dictionary`](crate::JapaneseEngine::with_dictionary).
//!
//! Directories searched, in this order (earlier entries win):
//!
//! 1. the user directory `$XDG_DATA_HOME/easime/dictionary`
//!    (default `~/.local/share/easime/dictionary`)
//! 2. the system directory `/usr/share/easime/dictionary`
//!
//! In each directory `*.tsv` files (the [`MemoryDictionary`] text format)
//! and `*.bin` files (the [`FileDictionary`] binary format) are loaded in
//! file name order. Other files are ignored. A directory that does not
//! exist is simply skipped; a file that cannot be loaded is reported in
//! [`LoadReport::failed`] and does not stop the others.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::dictionary::{Dictionary, MemoryDictionary};
use crate::file_dict::FileDictionary;

/// Where distribution packages install dictionaries.
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

/// The user dictionary directory for the given environment values.
/// `XDG_DATA_HOME` is only honoured when it is an absolute path, as the
/// XDG specification requires.
pub fn user_dictionary_dir_from(
    xdg_data_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Option<PathBuf> {
    let data_home = match xdg_data_home.map(Path::new) {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        _ => {
            let home = Path::new(home?);
            if home.as_os_str().is_empty() {
                return None;
            }
            home.join(".local/share")
        }
    };
    Some(data_home.join("easime").join("dictionary"))
}

/// The user dictionary directory according to the environment.
pub fn user_dictionary_dir() -> Option<PathBuf> {
    user_dictionary_dir_from(
        std::env::var_os("XDG_DATA_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

/// User directory first, then the system directory.
pub fn default_dictionary_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = user_dictionary_dir().into_iter().collect();
    dirs.push(PathBuf::from(SYSTEM_DICTIONARY_DIR));
    dirs
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

/// Load every `.tsv` and `.bin` dictionary in `dirs`, directories in the
/// order given (put the user directory first), files by name.
pub fn load_dictionaries(dirs: &[PathBuf]) -> LoadReport {
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
                Some("bin") => FileDictionary::open(&path)
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

    #[test]
    fn user_dir_follows_the_xdg_rules() {
        let dir = |x: Option<&OsStr>, h: Option<&OsStr>| user_dictionary_dir_from(x, h);
        // Absolute paths are platform specific ("/data" has no drive letter,
        // so Windows does not call it absolute): build them from the
        // temporary directory, which is absolute everywhere.
        let data = std::env::temp_dir().join("data");
        let home = std::env::temp_dir().join("home");
        let easime = |base: &Path| base.join("easime").join("dictionary");
        let from_home = easime(&home.join(".local").join("share"));

        assert_eq!(
            dir(Some(data.as_os_str()), Some(home.as_os_str())),
            Some(easime(&data))
        );
        assert_eq!(dir(None, Some(home.as_os_str())), Some(from_home.clone()));
        // A relative XDG_DATA_HOME is ignored.
        assert_eq!(
            dir(Some(OsStr::new("relative")), Some(home.as_os_str())),
            Some(from_home.clone())
        );
        assert_eq!(
            dir(Some(OsStr::new("")), Some(home.as_os_str())),
            Some(from_home)
        );
        assert_eq!(dir(None, None), None);
        assert_eq!(dir(None, Some(OsStr::new(""))), None);
    }

    #[test]
    fn default_dirs_end_with_the_system_dir() {
        let dirs = default_dictionary_dirs();
        assert_eq!(dirs.last(), Some(&PathBuf::from(SYSTEM_DICTIONARY_DIR)));
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
