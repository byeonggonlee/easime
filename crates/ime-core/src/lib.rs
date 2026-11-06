//! OS-independent core types for easime.

pub mod engine;
pub mod key;

pub use engine::{CandidateList, EngineOutput, InputEngine, Preedit};
pub use key::{Key, KeyEvent, Modifiers};
