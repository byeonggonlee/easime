//! Japanese input engine: romaji to kana (conversion to kanji comes later).

pub mod dictionary;
pub mod engine;
pub mod file_dict;
pub mod kana;
pub mod loader;
pub mod romaji;
pub mod table;

pub use dictionary::Dictionary;
pub use engine::{JapaneseEngine, KanaMode};
pub use file_dict::FileDictionary;
pub use loader::{LayeredDictionary, load_default, load_dictionaries};
