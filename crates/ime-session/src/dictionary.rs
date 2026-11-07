//! The Japanese dictionary, opened once and shared by every Japanese engine.
//!
//! Settings changes rebuild the engines, but the dictionary (tens of
//! megabytes, about 0.1 s to open) is only loaded once, when the first
//! Japanese engine is needed. Where the files are looked for differs per OS
//! (`engine_japanese::loader::default_dictionary_dirs`).

use engine_japanese::Dictionary;
use std::sync::Arc;
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

    /// The dictionary, loading it now if that has not happened yet. What was
    /// found (or not) goes to `log`.
    pub fn get(&mut self, log: &dyn Fn(&str)) -> Arc<dyn Dictionary> {
        self.loaded
            .get_or_insert_with(|| load_installed(log))
            .clone()
    }
}

/// Open the dictionaries in the user and system directories and say what was
/// found: with no dictionary conversion only offers kana, and the log is where
/// that can be told apart from "the word is not in it".
fn load_installed(log: &dyn Fn(&str)) -> Arc<dyn Dictionary> {
    let started = Instant::now();
    let dirs = engine_japanese::loader::default_dictionary_dirs();
    let report = engine_japanese::load_dictionaries(&dirs);
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
