//! Where the settings file lives and how it is read and written.

use crate::Config;
use ime_core::Os;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const APP_DIR: &str = "easime";
const FILE_NAME: &str = "config.toml";

type Env<'a> = &'a dyn Fn(&str) -> Option<OsString>;

fn absolute(value: Option<OsString>) -> Option<PathBuf> {
    value.map(PathBuf::from).filter(|p| p.is_absolute())
}

/// `$XDG_CONFIG_HOME` if absolute, else `$HOME/.config`.
fn linux_dir(env: Env) -> Option<PathBuf> {
    absolute(env("XDG_CONFIG_HOME")).or_else(|| absolute(env("HOME")).map(|h| h.join(".config")))
}

/// `%APPDATA%`.
fn windows_dir(env: Env) -> Option<PathBuf> {
    absolute(env("APPDATA"))
}

/// `$HOME/Library/Application Support`.
fn macos_dir(env: Env) -> Option<PathBuf> {
    absolute(env("HOME")).map(|h| h.join("Library").join("Application Support"))
}

/// The directory settings live in on `os`.
fn config_base_for(os: Os, env: Env) -> Option<PathBuf> {
    match os {
        Os::Windows => windows_dir(env),
        Os::MacOs => macos_dir(env),
        Os::Linux => linux_dir(env),
    }
}

/// The settings file path on `os`, or `None` if no home directory is known.
pub fn config_path_for(os: Os, env: Env) -> Option<PathBuf> {
    config_base_for(os, env).map(|base| base.join(APP_DIR).join(FILE_NAME))
}

/// The settings file path for this OS, or `None` if no home directory is known.
pub fn config_path() -> Option<PathBuf> {
    config_path_for(Os::current(), &|name| std::env::var_os(name))
}

/// The directory the log lives in on `os`: Linux the XDG state directory
/// (`$XDG_STATE_HOME`, default `~/.local/state`), macOS `~/Library/Logs`,
/// Windows `%LOCALAPPDATA%`.
fn log_base_for(os: Os, env: Env) -> Option<PathBuf> {
    match os {
        Os::Linux => absolute(env("XDG_STATE_HOME"))
            .or_else(|| absolute(env("HOME")).map(|h| h.join(".local").join("state"))),
        Os::MacOs => absolute(env("HOME")).map(|h| h.join("Library").join("Logs")),
        Os::Windows => absolute(env("LOCALAPPDATA")),
    }
}

/// The log file path on `os` (`easime/easime.log` below the log directory), or
/// `None` if no home directory is known.
pub fn log_path_for(os: Os, env: Env) -> Option<PathBuf> {
    log_base_for(os, env).map(|base| base.join(APP_DIR).join("easime.log"))
}

/// The log file path for this OS.
pub fn log_path() -> Option<PathBuf> {
    log_path_for(Os::current(), &|name| std::env::var_os(name))
}

/// Why a settings file could not be read or written.
#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Parse(toml::de::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "{e}"),
            StoreError::Parse(e) => write!(f, "invalid settings file: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl Config {
    /// Read settings from `path`. A missing file gives the defaults.
    pub fn load_from(path: &Path) -> Result<Config, StoreError> {
        match fs::read_to_string(path) {
            Ok(text) => Config::from_toml(&text).map_err(StoreError::Parse),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Write settings to `path`, creating directories as needed. The file is
    /// replaced atomically so a reader never sees a half-written file, and a
    /// symlink is followed so dotfiles managers keep their link.
    pub fn save_to(&self, path: &Path) -> Result<(), StoreError> {
        let target = if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            fs::canonicalize(path)?
        } else {
            path.to_path_buf()
        };
        if let Some(dir) = target.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut tmp = target.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        fs::write(&tmp, self.to_toml())?;
        if let Ok(meta) = fs::metadata(&target) {
            let _ = fs::set_permissions(&tmp, meta.permissions());
        }
        fs::rename(&tmp, &target)?;
        Ok(())
    }

    /// Load from the standard location, using defaults if there is none.
    pub fn load() -> Result<Config, StoreError> {
        match config_path() {
            Some(path) => Config::load_from(&path),
            None => Ok(Config::default()),
        }
    }

    /// Save to the standard location.
    pub fn save(&self) -> Result<(), StoreError> {
        let path = config_path().ok_or_else(|| {
            StoreError::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "no home directory to store settings in",
            ))
        })?;
        self.save_to(&path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BackspaceUnit, ToggleKey};
    use std::collections::HashMap;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect();
        move |name| map.get(name).cloned()
    }

    /// An absolute path that is valid on the OS running the tests.
    fn abs(path: &str) -> String {
        if cfg!(windows) {
            format!("C:{}", path.replace('/', "\\"))
        } else {
            path.to_string()
        }
    }

    fn abs_path(path: &str) -> PathBuf {
        PathBuf::from(abs(path))
    }

    #[test]
    fn linux_prefers_absolute_xdg_config_home() {
        let env = env_of(&[
            ("XDG_CONFIG_HOME", &abs("/x/cfg")),
            ("HOME", &abs("/home/u")),
        ]);
        assert_eq!(linux_dir(&env), Some(abs_path("/x/cfg")));
    }

    #[test]
    fn linux_ignores_empty_or_relative_xdg_config_home() {
        for bad in ["", "rel/path"] {
            let env = env_of(&[("XDG_CONFIG_HOME", bad), ("HOME", &abs("/home/u"))]);
            assert_eq!(linux_dir(&env), Some(abs_path("/home/u").join(".config")));
        }
    }

    #[test]
    fn windows_uses_appdata() {
        let env = env_of(&[("APPDATA", &abs("/Users/u/AppData/Roaming"))]);
        assert_eq!(
            windows_dir(&env),
            Some(abs_path("/Users/u/AppData/Roaming"))
        );
        assert_eq!(windows_dir(&env_of(&[])), None);
    }

    #[test]
    fn macos_uses_application_support() {
        let env = env_of(&[("HOME", &abs("/Users/u"))]);
        assert_eq!(
            macos_dir(&env),
            Some(
                abs_path("/Users/u")
                    .join("Library")
                    .join("Application Support")
            )
        );
    }

    #[test]
    fn no_home_means_no_path() {
        assert_eq!(linux_dir(&env_of(&[])), None);
        assert_eq!(macos_dir(&env_of(&[])), None);
    }

    #[test]
    fn each_os_has_its_own_settings_file() {
        let env = env_of(&[
            ("HOME", &abs("/home/u")),
            ("APPDATA", &abs("/Users/u/AppData/Roaming")),
        ]);
        let file = |os| config_path_for(os, &env);
        assert_eq!(
            file(Os::Linux),
            Some(
                abs_path("/home/u")
                    .join(".config")
                    .join("easime")
                    .join("config.toml")
            )
        );
        assert_eq!(
            file(Os::Windows),
            Some(
                abs_path("/Users/u/AppData/Roaming")
                    .join("easime")
                    .join("config.toml")
            )
        );
        assert_eq!(
            file(Os::MacOs),
            Some(
                abs_path("/home/u")
                    .join("Library")
                    .join("Application Support")
                    .join("easime")
                    .join("config.toml")
            )
        );
    }

    #[test]
    fn the_log_lives_where_each_os_keeps_logs() {
        let env = env_of(&[
            ("XDG_STATE_HOME", &abs("/x/state")),
            ("HOME", &abs("/home/u")),
            ("LOCALAPPDATA", &abs("/Users/u/AppData/Local")),
        ]);
        let log = |os| log_path_for(os, &env);
        assert_eq!(
            log(Os::Linux),
            Some(abs_path("/x/state").join("easime").join("easime.log"))
        );
        assert_eq!(
            log(Os::Windows),
            Some(
                abs_path("/Users/u/AppData/Local")
                    .join("easime")
                    .join("easime.log")
            )
        );
        assert_eq!(
            log(Os::MacOs),
            Some(
                abs_path("/home/u")
                    .join("Library")
                    .join("Logs")
                    .join("easime")
                    .join("easime.log")
            )
        );
    }

    #[test]
    fn the_linux_log_falls_back_to_the_home_directory() {
        for bad in ["", "rel/path"] {
            let env = env_of(&[("XDG_STATE_HOME", bad), ("HOME", &abs("/home/u"))]);
            assert_eq!(
                log_path_for(Os::Linux, &env),
                Some(
                    abs_path("/home/u")
                        .join(".local")
                        .join("state")
                        .join("easime")
                        .join("easime.log")
                )
            );
        }
        assert_eq!(log_path_for(Os::Linux, &env_of(&[])), None);
        assert_eq!(log_path_for(Os::Windows, &env_of(&[])), None);
        assert_eq!(log_path_for(Os::MacOs, &env_of(&[])), None);
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("easime-config-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn missing_file_loads_defaults() {
        let dir = scratch_dir("missing");
        assert_eq!(
            Config::load_from(&dir.join("nope.toml")).unwrap(),
            Config::default()
        );
    }

    #[test]
    fn save_then_load_round_trips_and_creates_directories() {
        let dir = scratch_dir("roundtrip");
        let path = dir.join("nested").join("config.toml");
        let mut c = Config::default();
        c.shortcuts.toggle_key = ToggleKey::RightAlt;
        c.korean.backspace = BackspaceUnit::Syllable;
        c.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path).unwrap(), c);
        assert!(!path.with_extension("toml.tmp").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn corrupt_file_is_a_parse_error() {
        let dir = scratch_dir("corrupt");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(&path, "this is = = not toml").unwrap();
        assert!(matches!(
            Config::load_from(&path),
            Err(StoreError::Parse(_))
        ));
        fs::remove_dir_all(&dir).unwrap();
    }
}
