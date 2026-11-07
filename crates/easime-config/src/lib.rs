//! Settings shared by every easime frontend and the settings apps.
//!
//! The file is TOML. Every field has a default and unknown fields are
//! ignored, so older and newer versions can share one file.

mod model;

pub use model::{BackspaceUnit, Config, Korean, Shortcuts, ToggleKey};
