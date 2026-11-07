//! OS-independent core types for easime.

pub mod engine;
pub mod key;
pub mod language;
pub mod os;

pub use engine::{CandidateList, EngineOutput, InputEngine, Preedit};
pub use key::{Key, KeyEvent, Modifiers};
pub use language::{Language, LanguageSwitcher};
pub use os::Os;
