//! Japanese input engine: romaji to kana and dictionary-based conversion.

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
pub use loader::{
    Access, LayeredDictionary, load_default, load_dictionaries, load_dictionaries_with,
};
