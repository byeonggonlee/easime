//! Whether the Japanese dictionary is there, for the General page.
//!
//! Without a dictionary Japanese conversion offers only kana, and until now the
//! only place that said so was the log. The page needs the state (found,
//! missing, unreadable) and where to look, not the file layout, so that is all
//! this returns.

use std::path::PathBuf;

/// What was found in the dictionary folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictionaryState {
    /// This many dictionary files are usable.
    Found(usize),
    /// Nothing there: the folders that were looked in.
    Missing(Vec<PathBuf>),
    /// Files exist but some could not be read (how many), with how many could.
    Unreadable { broken: usize, usable: usize },
}

impl DictionaryState {
    /// The word the page switches on.
    pub fn code(&self) -> &'static str {
        match self {
            DictionaryState::Found(_) => "found",
            DictionaryState::Missing(_) => "missing",
            DictionaryState::Unreadable { .. } => "unreadable",
        }
    }

    /// The number or the folders the page puts into its sentence.
    pub fn detail(&self) -> String {
        match self {
            DictionaryState::Found(count) => count.to_string(),
            DictionaryState::Missing(dirs) => dirs
                .iter()
                .map(|dir| dir.display().to_string())
                .collect::<Vec<_>>()
                .join("\n"),
            DictionaryState::Unreadable { broken, .. } => broken.to_string(),
        }
    }
}

/// Look in `dirs` the way the input method does.
pub fn inspect(dirs: &[PathBuf]) -> DictionaryState {
    let report = engine_japanese::load_dictionaries(dirs);
    let usable = report.loaded.len();
    let broken = report.failed.len();
    if broken > 0 {
        DictionaryState::Unreadable { broken, usable }
    } else if usable == 0 {
        DictionaryState::Missing(dirs.to_vec())
    } else {
        DictionaryState::Found(usable)
    }
}

/// The state in the folders easime uses on this OS.
pub fn inspect_installed() -> DictionaryState {
    inspect(&engine_japanese::loader::default_dictionary_dirs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn folder(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "easime-dictionary-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn no_folder_or_an_empty_one_is_missing() {
        let empty = folder("empty");
        let absent = empty.join("absent");
        let state = inspect(&[absent.clone(), empty.clone()]);
        assert_eq!(state, DictionaryState::Missing(vec![absent, empty.clone()]));
        assert_eq!(state.code(), "missing");
        assert_eq!(state.detail().lines().count(), 2, "one line per folder");
        let _ = fs::remove_dir_all(empty);
    }

    #[test]
    fn a_text_dictionary_is_found() {
        let dir = folder("found");
        fs::write(dir.join("words.tsv"), "にほん\t日本\n").unwrap();
        let state = inspect(std::slice::from_ref(&dir));
        assert_eq!(state, DictionaryState::Found(1));
        assert_eq!((state.code(), state.detail().as_str()), ("found", "1"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_broken_binary_dictionary_is_reported_not_hidden() {
        let dir = folder("broken");
        fs::write(dir.join("bad.bin"), b"not a dictionary").unwrap();
        fs::write(dir.join("good.tsv"), "にほん\t日本\n").unwrap();
        let state = inspect(std::slice::from_ref(&dir));
        assert_eq!(
            state,
            DictionaryState::Unreadable {
                broken: 1,
                usable: 1
            }
        );
        assert_eq!(state.code(), "unreadable");
        let _ = fs::remove_dir_all(dir);
    }
}
