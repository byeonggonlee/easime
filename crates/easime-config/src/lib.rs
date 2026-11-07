//! Settings shared by every easime frontend and the settings apps.
//!
//! The file is TOML. Every field has a default and unknown fields are
//! ignored, so older and newer versions can share one file.

mod model;
mod store;

pub use model::{Appearance, BackspaceUnit, Config, Korean, Languages, Shortcuts, ToggleKey};
pub use store::{StoreError, config_path};
