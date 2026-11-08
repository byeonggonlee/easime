//! The Japanese dictionary, opened once and shared by every Japanese engine.
//!
//! Settings changes rebuild the engines, but the dictionary (tens of
//! megabytes, about 0.1 s to open) is only loaded once, when the first
//! Japanese engine is needed. Where the files are looked for differs per OS
//! (`engine_japanese::loader::default_dictionary_dirs`).

use engine_japanese::Dictionary;
use engine_japanese::loader::Access;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

/// Loads the installed dictionaries on first use.
pub struct JapaneseDictionary {
    loaded: Option<Arc<dyn Dictionary>>,
}

impl JapaneseDictionary {
    /// Load the installed dictionaries when first needed.
    pub fn installed() -> Self {
        Self { loaded: None }
    }

    /// Use `dictionary` and never look at the disk (tests).
    pub fn fixed(dictionary: Arc<dyn Dictionary>) -> Self {
        Self {
            loaded: Some(dictionary),
        }
    }

    /// Open the dictionaries in `dirs` (the user's folder first) only when a
    /// word is first looked up, `access` saying how binary files are opened.
    /// For a frontend whose engines live in processes that may never convert
    /// a word: the Windows text service is loaded into every application, and
    /// most of them never see a conversion. What was found goes to `log` then.
    pub fn on_first_lookup(dirs: Vec<PathBuf>, access: Access, log: fn(&str)) -> Self {
        Self::fixed(Arc::new(OnFirstLookup {
            dirs,
            access,
            log,
            opened: OnceLock::new(),
        }))
    }

    /// The dictionary, loading it now if that has not happened yet. What was
    /// found (or not) goes to `log`.
    pub fn get(&mut self, log: &dyn Fn(&str)) -> Arc<dyn Dictionary> {
        self.loaded
            .get_or_insert_with(|| load_installed(log))
            .clone()
    }
}

/// See [`JapaneseDictionary::on_first_lookup`].
struct OnFirstLookup {
    dirs: Vec<PathBuf>,
    access: Access,
    log: fn(&str),
    opened: OnceLock<Arc<dyn Dictionary>>,
}

impl Dictionary for OnFirstLookup {
    fn lookup(&self, reading: &str) -> Vec<String> {
        self.opened
            .get_or_init(|| load(&self.dirs, self.access, &self.log))
            .lookup(reading)
    }
}

/// The dictionaries in the user and system directories of this OS, read.
fn load_installed(log: &dyn Fn(&str)) -> Arc<dyn Dictionary> {
    load(
        &engine_japanese::loader::default_dictionary_dirs(),
        Access::Read,
        log,
    )
}

/// Open the dictionaries in `dirs` and say what was found: with no dictionary
/// conversion only offers kana, and the log is where that can be told apart
/// from "the word is not in it".
fn load(dirs: &[PathBuf], access: Access, log: &dyn Fn(&str)) -> Arc<dyn Dictionary> {
    let started = Instant::now();
    let report = engine_japanese::load_dictionaries_with(dirs, access);
    for (path, why) in &report.failed {
        log(&format!(
            "japanese dictionary: could not load {}: {why}",
            path.display()
        ));
    }
    if report.skipped_lines > 0 {
        log(&format!(
            "japanese dictionary: skipped {} malformed lines in text files",
            report.skipped_lines
        ));
    }
    if report.loaded.is_empty() {
        let searched: Vec<_> = dirs.iter().map(|d| d.display().to_string()).collect();
        log(&format!(
            "japanese dictionary: none loaded (looked in {}); conversion offers only kana",
            searched.join(", ")
        ));
    } else {
        let files: Vec<_> = report
            .loaded
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        log(&format!(
            "japanese dictionary: loaded {} in {} ms",
            files.join(", "),
            started.elapsed().as_millis()
        ));
    }
    Arc::new(report.dictionary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_japanese::dictionary::MemoryDictionary;
    use std::cell::RefCell;

    #[test]
    fn a_fixed_dictionary_is_used_without_touching_the_disk() {
        let fixed: Arc<dyn Dictionary> = Arc::new(MemoryDictionary::parse("にほん\t日本\n").0);
        let mut dictionary = JapaneseDictionary::fixed(fixed);
        let messages = RefCell::new(Vec::new());
        let log = |m: &str| messages.borrow_mut().push(m.to_owned());
        assert_eq!(dictionary.get(&log).lookup("にほん"), ["日本"]);
        assert!(
            messages.borrow().is_empty(),
            "nothing was loaded, so nothing is logged"
        );
    }

    static OPENED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

    fn remember(message: &str) {
        OPENED.lock().unwrap().push(message.to_owned());
    }

    #[test]
    fn a_dictionary_opened_on_first_lookup_reads_nothing_before() {
        let dir = std::env::temp_dir().join(format!(
            "easime-session-first-lookup-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut dictionary =
            JapaneseDictionary::on_first_lookup(vec![dir.clone()], Access::Map, remember);
        let quiet = |_: &str| {};
        let shared = dictionary.get(&quiet);
        assert!(OPENED.lock().unwrap().is_empty(), "nothing opened yet");

        // The file appears only now, after the engine got its dictionary: it is
        // found, because nothing was read until the first word.
        std::fs::write(dir.join("words.tsv"), "にほん\t日本\n").unwrap();
        assert_eq!(shared.lookup("にほん"), ["日本"]);
        assert!(shared.lookup("ない").is_empty());
        let opened = OPENED.lock().unwrap().clone();
        assert_eq!(opened.len(), 1, "{opened:?}");
        assert!(
            opened[0].starts_with("japanese dictionary: loaded"),
            "{opened:?}"
        );
        // Once, for every engine that shares it.
        assert!(Arc::ptr_eq(&shared, &dictionary.get(&quiet)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_installed_dictionary_is_loaded_once_and_reported() {
        let mut dictionary = JapaneseDictionary::installed();
        let messages = RefCell::new(Vec::new());
        let log = |m: &str| messages.borrow_mut().push(m.to_owned());
        let first = dictionary.get(&log);
        let reported = messages.borrow().len();
        assert!(
            reported >= 1,
            "what was found, or that nothing was, is logged"
        );
        assert!(
            messages
                .borrow()
                .iter()
                .all(|m| m.starts_with("japanese dictionary:")),
            "{:?}",
            messages.borrow()
        );
        let second = dictionary.get(&log);
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            messages.borrow().len(),
            reported,
            "the second call loads nothing"
        );
    }
}
