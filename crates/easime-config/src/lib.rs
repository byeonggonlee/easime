//! Settings shared by every easime frontend and the settings apps.
//!
//! The file is TOML. Every field has a default and unknown fields are
//! ignored, so older and newer versions can share one file.

mod locale;
mod model;
mod store;

pub use locale::{
    AVAILABLE_LANGUAGES, language_for_tag, languages_for_tags, system_language_tags,
    system_language_tags_in, system_languages, system_languages_in,
};
pub use model::{Appearance, BackspaceUnit, Config, Korean, Languages, Shortcuts, ToggleKey};
pub use store::{StoreError, config_path, config_path_for, log_path, log_path_for};
