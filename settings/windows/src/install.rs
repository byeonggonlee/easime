//! Where the installed files are, seen from the settings app. The installer
//! puts the app and the Windows App Runtime it needs in a `settings` folder
//! inside the easime folder, next to the text service (`easime_tsf.dll`), so
//! the runtime's many files stay out of the way. The Japanese dictionary is
//! where the text service reads it: in `dictionary` next to its DLL
//! (`frontend_tsf::install::dictionary_dirs`).

use std::path::{Path, PathBuf};

/// The text service's file name, as installed.
pub const TEXT_SERVICE: &str = "easime_tsf.dll";

/// The folder of installed dictionaries, next to the text service.
pub const DICTIONARY_FOLDER: &str = "dictionary";

/// The installed dictionary folder for the app at `exe`: next to the text
/// service in the app's folder or the one above it (the installed layout),
/// else next to the app (a build tree). `exists` says whether a file is there.
pub fn installed_dictionary(exe: &Path, exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let folder = exe.parent().filter(|f| !f.as_os_str().is_empty())?;
    let beside_service = std::iter::successors(Some(folder), |f| f.parent())
        .take(2)
        .find(|f| exists(&f.join(TEXT_SERVICE)));
    Some(beside_service.unwrap_or(folder).join(DICTIONARY_FOLDER))
}

/// The Japanese dictionary folders the text service reads, as this app sees
/// them: the user's own first, then the installed one.
pub fn dictionary_dirs(user: Option<PathBuf>, exe: Option<&Path>) -> Vec<PathBuf> {
    let installed = exe.and_then(|exe| installed_dictionary(exe, |path| path.is_file()));
    user.into_iter().chain(installed).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn easime() -> PathBuf {
        std::env::temp_dir().join("Program Files").join("easime")
    }

    #[test]
    fn installed_the_dictionary_is_next_to_the_text_service_one_folder_up() {
        let exe = easime().join("settings").join("easime-settings.exe");
        let service = easime().join(TEXT_SERVICE);
        let found = installed_dictionary(&exe, |path| path == service);
        assert_eq!(found, Some(easime().join(DICTIONARY_FOLDER)));
    }

    #[test]
    fn next_to_the_text_service_in_the_same_folder_too() {
        let exe = easime().join("easime-settings.exe");
        let service = easime().join(TEXT_SERVICE);
        let found = installed_dictionary(&exe, |path| path == service);
        assert_eq!(found, Some(easime().join(DICTIONARY_FOLDER)));
    }

    #[test]
    fn without_a_text_service_it_is_next_to_the_app() {
        let exe = easime().join("target").join("easime-settings.exe");
        let found = installed_dictionary(&exe, |_| false);
        assert_eq!(found, Some(easime().join("target").join(DICTIONARY_FOLDER)));
        assert_eq!(
            installed_dictionary(Path::new("easime-settings.exe"), |_| false),
            None
        );
    }

    #[test]
    fn the_users_folder_comes_first() {
        let user = std::env::temp_dir().join("user-dictionary");
        let dirs = dictionary_dirs(Some(user.clone()), None);
        assert_eq!(dirs, vec![user]);
    }
}
