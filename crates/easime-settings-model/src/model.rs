//! What the settings apps show and what a change does to the settings
//! file. No UI toolkit in here, so all of it is tested without a display, on
//! every OS.
//!
//! The settings file is also edited by hand, so a change rewrites only the
//! keys that differ from the file: comments, key order and keys this version
//! does not know stay as they are. A file that cannot be read is never
//! overwritten by a change.

use easime_config::{AVAILABLE_LANGUAGES, BackspaceUnit, Config, ToggleKey};
use ime_core::Language;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Value};

/// The languages that can be switched on today. Chinese has no engine yet, so
/// offering it would show a choice that does nothing.
pub const SELECTABLE: [Language; 2] = AVAILABLE_LANGUAGES;

/// The name used in the settings file.
pub fn language_code(language: Language) -> &'static str {
    match language {
        Language::English => "english",
        Language::Korean => "korean",
        Language::Japanese => "japanese",
        Language::ChineseSimplified => "chinese_simplified",
        Language::ChineseTraditional => "chinese_traditional",
    }
}

pub fn language_from_code(code: &str) -> Option<Language> {
    [
        Language::English,
        Language::Korean,
        Language::Japanese,
        Language::ChineseSimplified,
        Language::ChineseTraditional,
    ]
    .into_iter()
    .find(|&language| language_code(language) == code)
}

pub fn toggle_key_code(key: ToggleKey) -> &'static str {
    match key {
        ToggleKey::Hangul => "hangul",
        ToggleKey::RightAlt => "right_alt",
        ToggleKey::Both => "both",
    }
}

pub fn toggle_key_from_code(code: &str) -> Option<ToggleKey> {
    [ToggleKey::Hangul, ToggleKey::RightAlt, ToggleKey::Both]
        .into_iter()
        .find(|&key| toggle_key_code(key) == code)
}

pub fn backspace_code(unit: BackspaceUnit) -> &'static str {
    match unit {
        BackspaceUnit::Jamo => "jamo",
        BackspaceUnit::Syllable => "syllable",
    }
}

pub fn backspace_from_code(code: &str) -> Option<BackspaceUnit> {
    [BackspaceUnit::Jamo, BackspaceUnit::Syllable]
        .into_iter()
        .find(|&unit| backspace_code(unit) == code)
}

/// Why a change or a reset did not happen. The window words the message
/// itself (so it can be translated); `detail` is the technical reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub kind: FailureKind,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// The settings file exists but cannot be read or understood. Nothing is
    /// written, so a hand edit is never lost.
    Unreadable,
    /// There is no home directory to keep settings in.
    NoHome,
    /// The new settings could not be written.
    WriteFailed,
    /// The request itself makes no sense (for example an unknown value).
    Invalid,
    /// Undoing the input method change was refused: another input method has
    /// been selected since, and that choice stays.
    ActivationChanged,
}

impl Failure {
    fn new(kind: FailureKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

/// The settings shown in the window, kept in step with the file.
pub struct Model {
    path: Option<PathBuf>,
    config: Config,
    /// The languages that match the system language: what is switched on
    /// until the user chooses.
    system: Vec<Language>,
    /// Set when the file could not be read; changes are refused meanwhile.
    problem: Option<Failure>,
}

impl Model {
    /// Read the settings at `path`. If that fails the defaults are shown and
    /// [`Model::problem`] says why.
    pub fn open(path: Option<PathBuf>) -> Self {
        Self::open_with_system(path, easime_config::system_languages())
    }

    /// Like [`Model::open`], with the system's languages given (for tests).
    pub fn open_with_system(path: Option<PathBuf>, system: Vec<Language>) -> Self {
        let (config, problem) = match &path {
            None => (
                Config::default(),
                Some(Failure::new(FailureKind::NoHome, "no home directory")),
            ),
            Some(path) => match Config::load_from(path) {
                Ok(config) => (config, None),
                Err(e) => (
                    Config::default(),
                    Some(Failure::new(FailureKind::Unreadable, e.to_string())),
                ),
            },
        };
        Self {
            path,
            config,
            system,
            problem,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Why the settings could not be loaded, if they could not.
    pub fn problem(&self) -> Option<&Failure> {
        self.problem.as_ref()
    }

    /// Read the file again, for example after it was edited by hand.
    pub fn reload(&mut self) {
        let system = std::mem::take(&mut self.system);
        *self = Self::open_with_system(self.path.take(), system);
    }

    /// The enabled languages that exist today, in switching order. What the
    /// daemon will actually cycle through after English.
    pub fn languages(&self) -> Vec<Language> {
        self.config
            .languages
            .resolved(&self.system)
            .into_iter()
            .filter(|language| SELECTABLE.contains(language))
            .collect()
    }

    /// True while the user has not chosen languages, so the system language
    /// decides which are on. The first change makes the choice explicit.
    pub fn follows_system_language(&self) -> bool {
        self.config.languages.follows_system()
    }

    pub fn set_language_enabled(
        &mut self,
        language: Language,
        enabled: bool,
    ) -> Result<(), Failure> {
        if !SELECTABLE.contains(&language) {
            return Err(Failure::new(FailureKind::Invalid, language_code(language)));
        }
        self.change_languages(|list| {
            let present = list.contains(&language);
            if enabled && !present {
                list.push(language);
            } else if !enabled && present {
                list.retain(|&l| l != language);
            }
        })
    }

    /// Move `language` one place earlier (`up`) or later in the switching
    /// order. Moving past either end does nothing.
    pub fn move_language(&mut self, language: Language, up: bool) -> Result<(), Failure> {
        self.change_languages(|list| {
            let Some(from) = list.iter().position(|&l| l == language) else {
                return;
            };
            let to = if up {
                from.checked_sub(1)
            } else {
                (from + 1 < list.len()).then_some(from + 1)
            };
            if let Some(to) = to {
                list.swap(from, to);
            }
        })
    }

    /// Edit the languages as they are now (the system's until the user has
    /// chosen). Only an edit that changes something makes the list the
    /// user's own choice and writes it to the file.
    fn change_languages(&mut self, edit: impl FnOnce(&mut Vec<Language>)) -> Result<(), Failure> {
        let current = self.config.languages.resolved(&self.system);
        let mut list = current.clone();
        edit(&mut list);
        if list == current {
            return Ok(());
        }
        self.change(|config| config.languages.enabled = Some(list))
    }

    pub fn set_toggle_key(&mut self, key: ToggleKey) -> Result<(), Failure> {
        self.change(|config| config.shortcuts.toggle_key = key)
    }

    pub fn set_backspace(&mut self, unit: BackspaceUnit) -> Result<(), Failure> {
        self.change(|config| config.korean.backspace = unit)
    }

    pub fn set_indicator_box(&mut self, shown: bool) -> Result<(), Failure> {
        self.change(|config| config.appearance.indicator_box = shown)
    }

    pub fn set_indicator_ms(&mut self, milliseconds: u32) -> Result<(), Failure> {
        self.change(|config| config.appearance.indicator_ms = milliseconds)
    }

    /// Replace the settings with the defaults. The old file is kept next to it
    /// with `.bak` added; its path is returned. This is the way out of a file
    /// that cannot be read.
    pub fn reset_to_defaults(&mut self) -> Result<Option<PathBuf>, Failure> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| Failure::new(FailureKind::NoHome, "no home directory"))?;
        let backup = if path.exists() {
            let mut name = path.as_os_str().to_owned();
            name.push(".bak");
            let backup = PathBuf::from(name);
            fs::copy(&path, &backup)
                .map_err(|e| Failure::new(FailureKind::WriteFailed, e.to_string()))?;
            Some(backup)
        } else {
            None
        };
        write_atomic(&path, &Config::default().to_toml())?;
        self.config = Config::default();
        self.problem = None;
        Ok(backup)
    }

    /// Apply `edit` to a copy of the settings and save it. The shown settings
    /// only change if saving worked.
    fn change(&mut self, edit: impl FnOnce(&mut Config)) -> Result<(), Failure> {
        let mut next = self.config.clone();
        edit(&mut next);
        if next == self.config {
            return Ok(());
        }
        if let Some(problem) = &self.problem {
            return Err(problem.clone());
        }
        let path = self
            .path
            .as_deref()
            .ok_or_else(|| Failure::new(FailureKind::NoHome, "no home directory"))?;
        save_changes(path, &next)?;
        self.config = next;
        Ok(())
    }
}

/// Bring the file in line with `next`, touching only the keys that differ
/// from what the file says now.
fn save_changes(path: &Path, next: &Config) -> Result<(), Failure> {
    let unreadable =
        |e: &dyn std::fmt::Display| Failure::new(FailureKind::Unreadable, e.to_string());
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(unreadable(&e)),
    };
    let on_disk = Config::from_toml(&text).map_err(|e| unreadable(&e))?;
    let mut doc: DocumentMut = text.parse().map_err(|e| unreadable(&e))?;

    // A language list is only written once the user has chosen one; until then
    // the key stays absent and the system language decides.
    if on_disk.languages != next.languages
        && let Some(chosen) = &next.languages.enabled
    {
        let mut codes = toml_edit::Array::new();
        for &language in chosen {
            codes.push(language_code(language));
        }
        set(&mut doc, "languages", "enabled", Value::Array(codes));
    }
    if on_disk.shortcuts != next.shortcuts {
        let code = toggle_key_code(next.shortcuts.toggle_key);
        set(&mut doc, "shortcuts", "toggle_key", Value::from(code));
    }
    if on_disk.korean != next.korean {
        let code = backspace_code(next.korean.backspace);
        set(&mut doc, "korean", "backspace", Value::from(code));
    }
    if on_disk.appearance.indicator_box != next.appearance.indicator_box {
        let shown = next.appearance.indicator_box;
        set(&mut doc, "appearance", "indicator_box", Value::from(shown));
    }
    if on_disk.appearance.indicator_ms != next.appearance.indicator_ms {
        let ms = i64::from(next.appearance.indicator_ms);
        set(&mut doc, "appearance", "indicator_ms", Value::from(ms));
    }
    write_atomic(path, &doc.to_string())
}

/// Set `table.key` in place. An existing key keeps its position and the
/// comments around it (including one after the value).
fn set(doc: &mut DocumentMut, table: &str, key: &str, mut value: Value) {
    let entry = doc.entry(table).or_insert_with(toml_edit::table);
    let Some(table) = entry.as_table_like_mut() else {
        return;
    };
    match table.get_mut(key) {
        Some(item) => {
            if let Some(old) = item.as_value() {
                *value.decor_mut() = old.decor().clone();
            }
            *item = Item::Value(value);
        }
        None => {
            table.insert(key, Item::Value(value));
        }
    }
}

/// Write the whole file at once: a reader (the daemon watches the file) never
/// sees a half-written one. A symbolic link is followed so dotfiles managers
/// keep their link, and existing permissions are kept.
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
            let _ = fs::set_permissions(&tmp, meta.permissions());
        }
        fs::rename(&tmp, &target)
    };
    write().map_err(|e| Failure::new(FailureKind::WriteFailed, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "easime-settings-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join("easime").join("config.toml")
        }

        fn write(&self, text: &str) {
            let path = self.file();
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }

        fn read(&self) -> String {
            fs::read_to_string(self.file()).unwrap()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    /// The system language the tests pretend to run under: Korean.
    const SYSTEM: [Language; 1] = [Language::Korean];

    fn open(dir: &TempDir) -> Model {
        Model::open_with_system(Some(dir.file()), SYSTEM.to_vec())
    }

    fn open_on_system(dir: &TempDir, system: &[Language]) -> Model {
        Model::open_with_system(Some(dir.file()), system.to_vec())
    }

    #[test]
    fn a_missing_file_shows_the_defaults_without_a_problem() {
        let dir = TempDir::new();
        let model = open(&dir);
        assert_eq!(model.config(), &Config::default());
        assert!(model.problem().is_none());
        assert!(!dir.file().exists(), "opening writes nothing");
    }

    #[test]
    fn no_home_directory_is_reported_and_nothing_can_be_saved() {
        let mut model = Model::open_with_system(None, SYSTEM.to_vec());
        assert_eq!(model.problem().unwrap().kind, FailureKind::NoHome);
        let err = model.set_indicator_box(false).unwrap_err();
        assert_eq!(err.kind, FailureKind::NoHome);
        assert!(model.config().appearance.indicator_box, "shown value kept");
    }

    #[test]
    fn a_change_creates_the_file_with_only_that_key() {
        let dir = TempDir::new();
        let mut model = open(&dir);
        model.set_backspace(BackspaceUnit::Syllable).unwrap();
        assert_eq!(dir.read(), "[korean]\nbackspace = \"syllable\"\n");
        assert_eq!(
            open(&dir).config().korean.backspace,
            BackspaceUnit::Syllable
        );
    }

    #[test]
    fn setting_what_is_already_set_writes_nothing() {
        let dir = TempDir::new();
        let mut model = open(&dir);
        model.set_toggle_key(ToggleKey::Both).unwrap();
        model.set_language_enabled(Language::Korean, true).unwrap();
        assert!(!dir.file().exists());
    }

    #[test]
    fn comments_order_and_unknown_keys_survive_a_change() {
        let dir = TempDir::new();
        let original = "\
# my settings
future_option = 7

[shortcuts]
# only the hangul key, I use right alt for something else
toggle_key = \"hangul\" # since 2026

[appearance]
indicator_box = true
extra = \"keep me\"
";
        dir.write(original);
        let mut model = open(&dir);
        model.set_indicator_ms(250).unwrap();
        model.set_toggle_key(ToggleKey::RightAlt).unwrap();
        let text = dir.read();
        assert!(text.contains("# my settings\nfuture_option = 7\n"));
        assert!(text.contains("# only the hangul key, I use right alt for something else\n"));
        assert!(
            text.contains("toggle_key = \"right_alt\" # since 2026\n"),
            "{text}"
        );
        assert!(text.contains("indicator_box = true\nextra = \"keep me\"\n"));
        assert!(text.contains("indicator_ms = 250"));
        let reread = open(&dir);
        assert_eq!(reread.config().appearance.indicator_ms, 250);
        assert_eq!(reread.config().shortcuts.toggle_key, ToggleKey::RightAlt);
    }

    #[test]
    fn only_the_changed_keys_are_written() {
        let dir = TempDir::new();
        dir.write("[appearance]\nindicator_ms = 400\n");
        let mut model = open(&dir);
        model.set_indicator_box(false).unwrap();
        assert_eq!(
            dir.read(),
            "[appearance]\nindicator_ms = 400\nindicator_box = false\n"
        );
    }

    #[test]
    fn languages_can_be_switched_on_off_and_reordered() {
        let dir = TempDir::new();
        let mut model = open(&dir);
        assert_eq!(model.languages(), [Language::Korean]);

        model
            .set_language_enabled(Language::Japanese, true)
            .unwrap();
        assert_eq!(model.languages(), [Language::Korean, Language::Japanese]);
        assert!(dir.read().contains("enabled = [\"korean\", \"japanese\"]"));

        model.move_language(Language::Japanese, true).unwrap();
        assert_eq!(model.languages(), [Language::Japanese, Language::Korean]);
        // Past either end nothing happens.
        model.move_language(Language::Japanese, true).unwrap();
        model.move_language(Language::Korean, false).unwrap();
        assert_eq!(model.languages(), [Language::Japanese, Language::Korean]);

        model.set_language_enabled(Language::Korean, false).unwrap();
        assert_eq!(model.languages(), [Language::Japanese]);
        assert_eq!(open(&dir).languages(), [Language::Japanese]);
    }

    #[test]
    fn a_japanese_only_setup_works_without_korean() {
        let dir = TempDir::new();
        let mut model = open(&dir);
        model
            .set_language_enabled(Language::Japanese, true)
            .unwrap();
        model.set_language_enabled(Language::Korean, false).unwrap();
        assert_eq!(dir.read(), "[languages]\nenabled = [\"japanese\"]\n");
        assert_eq!(
            open(&dir).config().languages.enabled,
            Some(vec![Language::Japanese])
        );
    }

    #[test]
    fn every_language_can_be_switched_off() {
        let dir = TempDir::new();
        let mut model = open(&dir);
        model.set_language_enabled(Language::Korean, false).unwrap();
        assert!(model.languages().is_empty());
        assert_eq!(dir.read(), "[languages]\nenabled = []\n");
        // Choosing nothing is a choice: the system language no longer decides.
        let reread = open(&dir);
        assert!(reread.languages().is_empty());
        assert!(!reread.follows_system_language());
    }

    #[test]
    fn languages_follow_the_system_until_the_user_chooses() {
        let dir = TempDir::new();
        let model = open_on_system(&dir, &[Language::Japanese]);
        assert_eq!(model.languages(), [Language::Japanese]);
        assert!(model.follows_system_language());

        // A system with no East Asian language starts with every language off.
        let model = open_on_system(&dir, &[]);
        assert!(model.languages().is_empty());
        assert!(model.follows_system_language());

        // The first real change keeps what was shown and makes it a choice.
        let mut model = open_on_system(&dir, &[Language::Japanese]);
        model.set_language_enabled(Language::Korean, true).unwrap();
        assert_eq!(model.languages(), [Language::Japanese, Language::Korean]);
        assert!(!model.follows_system_language());
        assert_eq!(
            dir.read(),
            "[languages]\nenabled = [\"japanese\", \"korean\"]\n"
        );
        // The file now wins over any system language.
        let reread = open_on_system(&dir, &[Language::Korean]);
        assert_eq!(reread.languages(), [Language::Japanese, Language::Korean]);
    }

    #[test]
    fn a_change_that_changes_nothing_does_not_make_a_choice() {
        let dir = TempDir::new();
        let mut model = open_on_system(&dir, &[Language::Korean]);
        // Korean is already on, and moving the only language goes nowhere.
        model.set_language_enabled(Language::Korean, true).unwrap();
        model.move_language(Language::Korean, true).unwrap();
        model.move_language(Language::Korean, false).unwrap();
        model
            .set_language_enabled(Language::Japanese, false)
            .unwrap();
        assert!(model.follows_system_language());
        assert!(!dir.file().exists());
    }

    #[test]
    fn other_changes_leave_the_language_choice_to_the_system() {
        let dir = TempDir::new();
        let mut model = open(&dir);
        model.set_backspace(BackspaceUnit::Syllable).unwrap();
        assert_eq!(dir.read(), "[korean]\nbackspace = \"syllable\"\n");
        assert!(open(&dir).follows_system_language());
    }

    #[test]
    fn languages_without_an_engine_are_not_listed_or_offered() {
        let dir = TempDir::new();
        dir.write("[languages]\nenabled = [\"chinese_simplified\", \"japanese\", \"english\"]\n");
        let mut model = open(&dir);
        assert_eq!(model.languages(), [Language::Japanese]);
        for language in [Language::ChineseSimplified, Language::ChineseTraditional] {
            let err = model.set_language_enabled(language, true).unwrap_err();
            assert_eq!(err.kind, FailureKind::Invalid);
        }
        // Hand-written entries stay in the file when something else changes.
        model.set_language_enabled(Language::Korean, true).unwrap();
        assert!(dir.read().contains("\"chinese_simplified\""));
    }

    #[test]
    fn an_unreadable_file_is_reported_and_never_overwritten() {
        let dir = TempDir::new();
        let broken = "[shortcuts\ntoggle_key = \"hangul\"\n# precious notes\n";
        dir.write(broken);
        let mut model = open(&dir);
        assert_eq!(model.problem().unwrap().kind, FailureKind::Unreadable);
        assert_eq!(model.config(), &Config::default());
        assert_eq!(model.languages(), [Language::Korean], "the system's");

        let err = model.set_indicator_box(false).unwrap_err();
        assert_eq!(err.kind, FailureKind::Unreadable);
        assert_eq!(dir.read(), broken, "the file is untouched");
        assert!(model.config().appearance.indicator_box);
    }

    #[test]
    fn a_wrong_value_in_the_file_counts_as_unreadable() {
        let dir = TempDir::new();
        dir.write("[shortcuts]\ntoggle_key = \"capslock\"\n");
        assert_eq!(open(&dir).problem().unwrap().kind, FailureKind::Unreadable);
    }

    #[test]
    fn fixing_the_file_by_hand_and_reloading_clears_the_problem() {
        let dir = TempDir::new();
        dir.write("[shortcuts\n");
        let mut model = open(&dir);
        assert!(model.problem().is_some());
        dir.write("[shortcuts]\ntoggle_key = \"hangul\"\n");
        model.reload();
        assert!(model.problem().is_none());
        assert_eq!(model.config().shortcuts.toggle_key, ToggleKey::Hangul);
        model.set_indicator_ms(300).unwrap();
        assert!(dir.read().contains("indicator_ms = 300"));
    }

    #[test]
    fn resetting_keeps_a_backup_and_clears_a_problem() {
        let dir = TempDir::new();
        let broken = "[shortcuts\n# notes\n";
        dir.write(broken);
        let mut model = open(&dir);
        let backup = model.reset_to_defaults().unwrap().expect("a backup");
        assert_eq!(fs::read_to_string(backup).unwrap(), broken);
        assert!(model.problem().is_none());
        assert_eq!(open(&dir).config(), &Config::default());
        model.set_indicator_box(false).unwrap();
    }

    #[test]
    fn resetting_without_a_file_makes_no_backup() {
        let dir = TempDir::new();
        let mut model = open(&dir);
        assert_eq!(model.reset_to_defaults().unwrap(), None);
        assert_eq!(open(&dir).config(), &Config::default());
    }

    #[test]
    fn a_failed_write_leaves_the_shown_settings_alone() {
        let dir = TempDir::new();
        // A directory where the file should be: reading and writing both fail.
        fs::create_dir_all(dir.file()).unwrap();
        let mut model = open(&dir);
        assert!(model.set_indicator_box(false).is_err());
        assert!(model.config().appearance.indicator_box);
    }

    #[test]
    fn codes_round_trip() {
        for language in SELECTABLE {
            assert_eq!(language_from_code(language_code(language)), Some(language));
        }
        for key in [ToggleKey::Hangul, ToggleKey::RightAlt, ToggleKey::Both] {
            assert_eq!(toggle_key_from_code(toggle_key_code(key)), Some(key));
        }
        for unit in [BackspaceUnit::Jamo, BackspaceUnit::Syllable] {
            assert_eq!(backspace_from_code(backspace_code(unit)), Some(unit));
        }
        assert_eq!(language_from_code("klingon"), None);
        assert_eq!(toggle_key_from_code("capslock"), None);
        assert_eq!(backspace_from_code("word"), None);
    }
}
