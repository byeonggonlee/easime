//! Japanese input engine: romaji to kana (conversion to kanji comes later).

pub mod engine;
pub mod kana;
pub mod romaji;
pub mod table;

pub use engine::{JapaneseEngine, KanaMode};
