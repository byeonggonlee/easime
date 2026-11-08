//! Where the installed files are. The installer puts everything in one folder,
//! which the user may choose (`packaging/windows/easime.wxs`), so the text
//! service finds its files next to its own DLL rather than at a fixed path.

use std::path::{Path, PathBuf};

/// The folder, next to the DLL, that holds the installed dictionaries.
pub const DICTIONARY_FOLDER: &str = "dictionary";

/// The Japanese dictionary folders the text service reads: the user's own
/// first (words added there come first), then the installed one next to the
/// DLL at `dll`.
pub fn dictionary_dirs(user: Option<PathBuf>, dll: Option<&Path>) -> Vec<PathBuf> {
    let installed = dll
        .and_then(Path::parent)
        .filter(|folder| !folder.as_os_str().is_empty())
        .map(|folder| folder.join(DICTIONARY_FOLDER));
    user.into_iter().chain(installed).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_installed_dictionary_is_next_to_the_dll_after_the_users() {
        let install = std::env::temp_dir()
            .join("Somewhere I chose")
            .join("easime");
        let dll = install.join("easime_tsf.dll");
        let user = std::env::temp_dir().join("user").join("dictionary");
        assert_eq!(
            dictionary_dirs(Some(user.clone()), Some(&dll)),
            [user.clone(), install.join("dictionary")]
        );
        assert_eq!(
            dictionary_dirs(None, Some(&dll)),
            [install.join("dictionary")]
        );
        assert_eq!(dictionary_dirs(Some(user.clone()), None), [user]);
        assert!(dictionary_dirs(None, None).is_empty());
        // A bare file name says nothing about where it is.
        assert!(dictionary_dirs(None, Some(Path::new("easime_tsf.dll"))).is_empty());
    }
}
